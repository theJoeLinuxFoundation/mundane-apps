use super::Files;
use gpui::{prelude::*, *};
use mundane_services::files::{self, Entry};
use mundane_ui::Theme;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    ops::Range,
    path::PathBuf,
    sync::Arc,
};

#[derive(Default)]
pub(super) struct Previews {
    ready: HashMap<PathBuf, Option<ImageSource>>,
    order: VecDeque<PathBuf>,
    pending: HashSet<PathBuf>,
}

impl Files {
    fn request_preview(&mut self, entry: &Entry, cx: &mut Context<Self>) {
        let path = &entry.path;
        if self.loading
            || self.previews.pending.len() >= 2
            || self.previews.ready.contains_key(path)
            || !self.previews.pending.insert(path.clone())
        {
            return;
        }
        let entry = entry.clone();
        let path = path.clone();
        let generation = self.generation;
        enum Visual {
            Thumbnail(Vec<u8>),
            Icon(PathBuf),
        }
        let work = cx.background_executor().spawn(async move {
            if !entry.folder
                && let Some(png) = files::preview(&entry.path)
            {
                Some(Visual::Thumbnail(png))
            } else {
                files::icon(&entry).map(Visual::Icon)
            }
        });
        cx.spawn(async move |view, cx| {
            let preview = work.await.map(|v| match v {
                Visual::Thumbnail(png) => {
                    ImageSource::Image(Arc::new(Image::from_bytes(ImageFormat::Png, png)))
                }
                Visual::Icon(path) => path.into(),
            });
            let _ = view.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                this.previews.pending.remove(&path);
                this.previews.ready.insert(path.clone(), preview);
                this.previews.order.push_back(path);
                if this.previews.order.len() > 256
                    && let Some(old) = this.previews.order.pop_front()
                {
                    this.previews.ready.remove(&old);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Stateful<Div>> {
        let theme = Theme::current(cx);
        range
            .map(|i| {
                let entry = self.entries[i].clone();
                self.request_preview(&entry, cx);
                let visual = self
                    .previews
                    .ready
                    .get(&entry.path)
                    .and_then(Option::as_ref);
                let selected = self.selected == Some(i);
                let detail = if entry.symlink {
                    "Link".into()
                } else if entry.folder {
                    "Folder".into()
                } else if entry.bytes < 1024 {
                    format!("{} B", entry.bytes)
                } else {
                    format!("{:.1} KiB", entry.bytes as f64 / 1024.)
                };
                let name: SharedString = entry.name.to_string_lossy().into_owned().into();
                let relative_parent = entry
                    .path
                    .parent()
                    .and_then(|p| p.strip_prefix(&self.path).ok())
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                mundane_ui::option(
                    ElementId::Path(entry.path.clone().into()),
                    name.clone(),
                    i,
                    selected,
                    self.list_focus.is_focused(window),
                )
                .when(window.is_a11y_active(), |el| {
                    el.aria_description(detail.clone())
                })
                .w_full()
                .h(px(46.))
                .px_3()
                .flex()
                .items_center()
                .gap_3()
                .when(selected, |el| {
                    el.bg(rgb(theme.selection))
                        .border_l_2()
                        .border_color(rgb(theme.focus))
                })
                .when(!selected, |el| el.hover(move |s| s.bg(rgb(theme.panel))))
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    if this.loading || this.working || this.dialog.is_some() {
                        return;
                    }
                    let Some(index) = this.entries.iter().position(|item| item.path == entry.path)
                    else {
                        return;
                    };
                    this.selected = Some(index);
                    this.message = None;
                    this.list_focus.focus(window, cx);
                    if event.click_count() == 2 {
                        this.activate(window, cx);
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .size(px(32.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when_some(visual.cloned(), |el, img_source| {
                            el.child(img(img_source).object_fit(ObjectFit::Contain).size_full())
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().child(name))
                        .when(self.nav.query.is_some(), |el| {
                            el.child(div().text_sm().truncate().child(relative_parent))
                        }),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(theme.secondary))
                        .child(detail),
                )
            })
            .collect()
    }
}
