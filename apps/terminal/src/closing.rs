use super::*;

#[derive(Clone, Copy)]
pub(super) enum Target {
    Tab(u64),
    Window,
}
pub(super) struct Warning {
    target: Target,
    jobs: Vec<String>,
    pub(super) buttons: [FocusHandle; 2],
}
impl Terminal {
    pub fn allow_close(&mut self, target: Target, w: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.warning.is_some() {
            return false;
        }
        let jobs: Vec<_> = self
            .tabs
            .iter()
            .filter(|t| match target {
                Target::Window => true,
                Target::Tab(id) => id == t.id,
            })
            .filter_map(|t| {
                t.session
                    .running_job()
                    .map(|job| format!("{} · {job}", t.label))
            })
            .collect();
        if jobs.is_empty() {
            return true;
        }
        let buttons = std::array::from_fn(|_| cx.focus_handle().tab_stop(true));
        self.find = None;
        self.form = None;
        buttons[0].focus(w, cx);
        self.warning = Some(Warning {
            target,
            jobs,
            buttons,
        });
        cx.notify();
        false
    }
    pub fn request_close(&mut self, target: Target, w: &mut Window, cx: &mut Context<Self>) {
        if self.allow_close(target, w, cx) {
            self.finish_close(target, w, cx);
        }
    }
    fn finish_close(&mut self, target: Target, w: &mut Window, cx: &mut Context<Self>) {
        match target {
            Target::Window => w.remove_window(),
            Target::Tab(id) => {
                if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
                    self.close_tab(i, w, cx);
                }
            }
        }
    }
    pub fn answer_close(&mut self, close: bool, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(warning) = self.warning.take() {
            if close {
                self.finish_close(warning.target, w, cx);
            } else {
                self.grid_focus.focus(w, cx);
            }
            cx.notify();
        }
    }
    pub fn warning_dialog(&self, warn: &Warning, cx: &Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        mundane_ui::Role::Dialog
            .control("job-warning", "Close running jobs?")
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000070))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(400.))
                    .max_w_full()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(t.panel))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(mundane_ui::heading("job-title", "Close running jobs?"))
                    .child("Closing the terminal can stop these jobs.")
                    .child(
                        div()
                            .id("active-jobs")
                            .max_h(px(150.))
                            .overflow_y_scroll()
                            .children(warn.jobs.clone()),
                    )
                    .child(
                        div().flex().gap_2().justify_end().children(
                            ["Cancel", "Close anyway"]
                                .into_iter()
                                .enumerate()
                                .map(|(i, label)| {
                                    let v = cx.weak_entity();
                                    button(label, &warn.buttons[i], t, move |w, cx| {
                                        let _ = v.update(cx, |s, cx| s.answer_close(i == 1, w, cx));
                                    })
                                }),
                        ),
                    ),
            )
    }
}
