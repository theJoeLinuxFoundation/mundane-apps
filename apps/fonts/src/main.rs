mod catalog;
use catalog::{Face, Family, Output, Renderer, Request};
use gpui::{prelude::*, *};
use mundane_ui::{
    BrowserList, BrowserRow, ControlRole, Role, TextField, Theme, WindowHeader, browser_key,
    browser_window, button, text_label, unavailable,
};
use std::path::PathBuf;

const SAMPLE: &str = "The quick brown fox";
enum Reply {
    Inventory(Result<Vec<Family>, String>),
    Preview(u64, Result<Output, String>),
}

async fn worker(
    dir: Option<PathBuf>,
    jobs: async_channel::Receiver<(u64, Request)>,
    replies: async_channel::Sender<Reply>,
) {
    if replies
        .send(Reply::Inventory(catalog::installed(dir.as_deref())))
        .await
        .is_err()
    {
        return;
    }
    // Pango objects stay on their creating thread, and edits reuse the selected face.
    let mut renderer: Option<(Face, Renderer)> = None;
    while let Ok((generation, request)) = jobs.recv().await {
        let result = (|| {
            if renderer.as_ref().is_none_or(|(f, _)| f != &request.face) {
                renderer = Some((request.face.clone(), Renderer::open(&request.face)?))
            }
            renderer.as_ref().unwrap().1.draw(&request)
        })();
        if replies
            .send(Reply::Preview(generation, result))
            .await
            .is_err()
        {
            break;
        }
    }
}
struct Fonts {
    header: WindowHeader,
    list: BrowserList,
    families: Vec<Family>,
    visible: Vec<usize>,
    face_index: usize,
    search: Entity<TextField>,
    sample: Entity<TextField>,
    prev: FocusHandle,
    next: FocusHandle,
    edit: FocusHandle,
    jobs: async_channel::Sender<(u64, Request)>,
    generation: u64,
    requested: Option<Request>,
    preview: Option<Output>,
    status: String,
    filter: String,
}
impl Fonts {
    fn new(dir: Option<PathBuf>, w: &mut Window, cx: &mut Context<Self>) -> Self {
        mundane_ui::bind_text_keys(cx);
        let theme = Theme::current(cx);
        let search = cx.new(|cx| TextField::new("Find a font family", theme, cx));
        let sample =
            cx.new(|cx| TextField::new("Sample text", theme, cx).select_value(SAMPLE.into()));
        cx.observe(&search, |this, _, cx| {
            let q = this.search.read(cx).text().trim().to_lowercase();
            if q != this.filter {
                this.filter = q;
                this.filter_rows(cx)
            }
        })
        .detach();
        cx.observe(&sample, |_, _, cx| cx.notify()).detach();
        let (jobs, incoming) = async_channel::bounded(1);
        let (replies, events) = async_channel::bounded(2);
        cx.background_executor()
            .scheduler_executor()
            .spawn_dedicated(move |_| worker(dir, incoming, replies))
            .detach();
        cx.spawn_in(w, async move |v, cx| {
            while let Ok(event) = events.recv().await {
                if v.update_in(cx, |this, w, cx| this.receive(event, w, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let list = BrowserList::new(cx);
        list.focus.focus(w, cx);
        Self {
            header: WindowHeader::new(cx),
            list,
            families: vec![],
            visible: vec![],
            face_index: 0,
            search,
            sample,
            prev: cx.focus_handle().tab_stop(true),
            next: cx.focus_handle().tab_stop(true),
            edit: cx.focus_handle().tab_stop(true),
            jobs,
            generation: 0,
            requested: None,
            preview: None,
            status: "Reading installed fonts…".into(),
            filter: String::new(),
        }
    }
    fn filter_rows(&mut self, cx: &mut Context<Self>) {
        self.visible = self
            .families
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.name.to_lowercase().contains(&self.filter).then_some(i))
            .collect();
        let rows = self
            .visible
            .iter()
            .map(|&i| {
                let f = &self.families[i];
                let count = f.faces.len();
                BrowserRow {
                    key: f.name.clone().into(),
                    label: f.name.clone(),
                    detail: format!("{count} {}", if count == 1 { "face" } else { "faces" }),
                    value: String::new(),
                    fraction: None,
                    depth: 0,
                }
            })
            .collect();
        self.list.replace(rows);
        self.face_index = 0;
        if self.visible.is_empty() {
            self.status = "No matching font families.".into()
        }
        cx.notify()
    }
    fn family(&self) -> Option<&Family> {
        self.list
            .selected
            .and_then(|i| self.visible.get(i))
            .and_then(|&i| self.families.get(i))
    }
    fn face(&self) -> Option<&Face> {
        self.family().and_then(|f| f.faces.get(self.face_index))
    }
    fn change_face(&mut self, next: bool, cx: &mut Context<Self>) {
        if let Some(f) = self.family() {
            self.face_index = if next {
                (self.face_index + 1).min(f.faces.len() - 1)
            } else {
                self.face_index.saturating_sub(1)
            };
            cx.notify()
        }
    }
    fn edit(&self, w: &mut Window, cx: &mut Context<Self>) {
        self.sample.focus_handle(cx).focus(w, cx);
        self.sample
            .update(cx, |f, cx| f.select_match(0..f.text().len(), cx))
    }
    fn receive(&mut self, reply: Reply, w: &mut Window, cx: &mut Context<Self>) {
        match reply {
            Reply::Inventory(Ok(families)) => {
                self.families = families;
                self.filter_rows(cx)
            }
            Reply::Inventory(Err(e)) => self.status = e,
            Reply::Preview(generation, result) if generation == self.generation => match result {
                Ok(p) => {
                    self.status = if p.missing.is_empty() {
                        "All sample characters covered.".into()
                    } else {
                        let codes = p
                            .missing
                            .iter()
                            .map(|&c| format!("U+{:04X}", c as u32))
                            .collect::<Vec<_>>();
                        format!("Missing glyphs · {}", codes.join(", "))
                    };
                    if p.clipped {
                        self.status.push_str(" Preview shortened to fit.")
                    }
                    if let Some(family) = self.family() {
                        w.set_window_title(&format!(
                            "{} · {} — Fonts",
                            family.name, family.faces[self.face_index].style
                        ));
                    }
                    self.preview = Some(p);
                }
                Err(e) => self.status = e,
            },
            _ => return,
        };
        w.refresh();
        cx.notify()
    }
    fn request(&mut self, w: &mut Window, cx: &App) {
        let theme = Theme::current(cx);
        let scale = w.scale_factor();
        let rem = f32::from(w.rem_size());
        let width = f32::from(w.viewport_size().width) * 0.62 - rem * 1.5;
        let next = self.face().map(|face| Request {
            face: face.clone(),
            sample: self.sample.read(cx).text().into(),
            size: (rem * 2. * scale).round().clamp(1., 512.) as i32,
            width: (width * scale).round().clamp(1., 4096.) as i32,
            height: (rem * 6. * scale).round().clamp(1., 2048.) as i32,
            foreground: theme.text,
            background: theme.panel,
        });
        if self.requested == next {
            return;
        }
        self.generation += 1;
        if let Some(old) = self.preview.take() {
            let _ = w.drop_image(old.image);
        }
        if let Some(r) = &next {
            self.status = "Drawing preview…".into();
            if self.jobs.force_send((self.generation, r.clone())).is_err() {
                self.status = "Font preview service is unavailable.".into()
            }
        }
        self.requested = next;
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        match (k.key.as_str(), k.modifiers.control) {
            ("f6", false) => self.edit.focus(w, cx),
            ("f", true) => {
                self.search.focus_handle(cx).focus(w, cx);
                self.search
                    .update(cx, |f, cx| f.select_match(0..f.text().len(), cx))
            }
            ("e", true) => self.edit(w, cx),
            ("enter" | "escape", false)
                if self.sample.focus_handle(cx).is_focused(w)
                    || self.search.focus_handle(cx).is_focused(w) =>
            {
                self.list.focus.focus(w, cx)
            }
            ("left" | "right", false) if self.list.focus.is_focused(w) => {
                self.change_face(k.key == "right", cx)
            }
            _ if self.list.focus.is_focused(w) && self.list.navigate(&k.key) => {
                self.face_index = 0;
                cx.notify()
            }
            _ if browser_key(k, w, cx) => {}
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation()
    }
}
impl Render for Fonts {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.request(w, cx);
        let t = Theme::current(cx);
        let v = cx.weak_entity();
        let edit = button("Edit sample", &self.edit, t, move |w, cx| {
            let _ = v.update(cx, |v, cx| v.edit(w, cx));
        });
        let v = cx.weak_entity();
        let list = self.list.render("Font families", t, w, move |i, _, w, cx| {
            let _ = v.update(cx, |v, cx| {
                v.list.select(i);
                v.face_index = 0;
                v.list.focus.focus(w, cx);
                cx.notify()
            });
        });
        let name = self
            .family()
            .zip(self.face())
            .map(|(f, face)| format!("{} · {}", f.name, face.style))
            .unwrap_or_else(|| "Font preview".into());
        let mut buttons = div().flex().gap_2();
        for (label, focus, forward, disabled) in [
            ("Previous face", &self.prev, false, self.face_index == 0),
            (
                "Next face",
                &self.next,
                true,
                self.family()
                    .is_none_or(|f| self.face_index + 1 >= f.faces.len()),
            ),
        ] {
            let v = cx.weak_entity();
            buttons = buttons.child(unavailable(
                button(label, focus, t, move |_, cx| {
                    if !disabled {
                        let _ = v.update(cx, |v, cx| v.change_face(forward, cx));
                    }
                }),
                disabled,
            ))
        }
        let sample = self.sample.read(cx).text().to_owned();
        let mut panel = div()
            .w(relative(0.62))
            .min_w_0()
            .bg(rgb(t.panel))
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                text_label("face", name.clone())
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name.clone()),
            )
            .child(buttons)
            .child(text_label("sample-label", "Sample text").child("Sample text"))
            .child(self.sample.clone());
        if let Some(p) = &self.preview {
            panel = panel.child(
                Role::Image
                    .control("sample-preview", format!("Preview in {name}"))
                    .aria_description(sample)
                    .w_full()
                    .h(rems(6.))
                    .child(
                        img(p.image.clone())
                            .size_full()
                            .object_fit(ObjectFit::Contain),
                    ),
            )
        }
        div()
            .size_full()
            .font_family("DejaVu Sans")
            .bg(rgb(t.surface))
            .text_color(rgb(t.text))
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::keys))
            .child(self.header.render("Fonts", edit, &self.list.focus, t, w))
            .child(div().p_3().child(self.search.clone()))
            .child(div().flex_1().min_h_0().flex().child(list).child(panel))
            .child(
                Role::Status
                    .control("coverage", self.status.clone())
                    .bg(rgb(t.panel))
                    .px_3()
                    .py_2()
                    .child(self.status.clone()),
            )
    }
}
fn main() {
    browser_window("Fonts", "org.joelinux.mundane.Fonts", |w, cx| {
        Fonts::new(None, w, cx)
    })
}
#[cfg(test)]
mod tests {
    use super::{Fonts, catalog::FIXTURE};
    use gpui::TestAppContext;
    #[gpui::test]
    fn switch_face_and_replace_sample(cx: &mut TestAppContext) {
        let (v, ui) = cx.add_window_view(|w, cx| Fonts::new(Some(FIXTURE.into()), w, cx));
        ui.run_until_parked();
        ui.simulate_keystrokes("right ctrl-e shift-a enter");
        ui.run_until_parked();
        v.read_with(ui, |f, cx| {
            assert_eq!(f.face().unwrap().style, "Bold");
            assert_eq!(f.sample.read(cx).text(), "A");
            assert!(f.preview.as_ref().unwrap().missing.is_empty())
        });
    }
}
