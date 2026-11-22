use super::*;
/// Owns focus and visibility for an inline popup. Call dismiss on an outside-dismiss
/// notification from the owning surface; the contents never install a global pointer handler.
pub struct Popover {
    trigger: FocusHandle,
    open: bool,
}
impl Popover {
    pub fn new(cx: &App) -> Self {
        Self {
            trigger: cx.focus_handle().tab_stop(true),
            open: false,
        }
    }
    pub fn trigger(&self) -> FocusHandle {
        self.trigger.clone()
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn show(&mut self, initial: &FocusHandle, w: &mut Window, cx: &mut App) {
        self.open = true;
        initial.focus(w, cx);
    }
    pub fn dismiss(&mut self, w: &mut Window, cx: &mut App) {
        if self.open {
            self.open = false;
            self.trigger.focus(w, cx);
        }
    }
    pub fn toggle(&mut self, initial: &FocusHandle, w: &mut Window, cx: &mut App) {
        if self.open {
            self.dismiss(w, cx)
        } else {
            self.show(initial, w, cx)
        }
    }
}
/// Supply the same dismiss callback for Escape and for the compositor's dismissal signal.
pub fn popover_contents(
    name: impl Into<SharedString>,
    t: Theme,
    body: impl IntoElement,
    dismiss: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    Role::Group
        .control("popup-content", name)
        .bg(rgb(t.panel))
        .rounded(tokens::radius::DIALOG)
        .border_1()
        .border_color(rgb(t.border))
        .p(tokens::space::SM)
        .on_key_down(move |e, w, cx| {
            if e.keystroke.key == "escape" {
                dismiss(w, cx);
                w.prevent_default();
                cx.stop_propagation();
            }
        })
        .child(body)
}
