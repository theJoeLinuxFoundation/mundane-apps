use crate::{Theme, button_base};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Control {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn controls(
    decorations: Decorations,
    caps: WindowControls,
    maximized: bool,
    fullscreen: bool,
) -> Vec<Control> {
    if fullscreen || decorations == Decorations::Server {
        return Vec::new();
    }
    let mut visible = Vec::with_capacity(3);
    if caps.minimize {
        visible.push(Control::Minimize);
    }
    if caps.maximize {
        visible.push(if maximized {
            Control::Restore
        } else {
            Control::Maximize
        });
    }
    visible.push(Control::Close);
    visible
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct WindowHeader {
    close_handler: CloseHandler,
    minimize: FocusHandle,
    maximize: FocusHandle,
    close: FocusHandle,
    drag_origin: Rc<Cell<Option<Point<Pixels>>>>,
}

impl WindowHeader {
    pub fn new(cx: &App) -> Self {
        Self {
            close_handler: Rc::new(|window, _| window.remove_window()),
            minimize: cx.focus_handle().tab_stop(true),
            maximize: cx.focus_handle().tab_stop(true),
            close: cx.focus_handle().tab_stop(true),
            drag_origin: Default::default(),
        }
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.close_handler = Rc::new(handler);
        self
    }

    pub fn render(
        &self,
        title: &'static str,
        actions: impl IntoElement,
        content_focus: &FocusHandle,
        theme: Theme,
        window: &Window,
    ) -> Div {
        let visible = controls(
            window.window_decorations(),
            window.window_controls(),
            window.is_maximized(),
            window.is_fullscreen(),
        );
        let can_zoom = visible.contains(&Control::Maximize) || visible.contains(&Control::Restore);
        let drag = self.drag_origin.clone();
        let motion = drag.clone();
        let titlebar = div()
            .id("window-title")
            .h_full()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .font_weight(FontWeight::SEMIBOLD)
            .child(title)
            .when(!visible.is_empty(), |el| {
                el.on_mouse_down(MouseButton::Left, move |e, window, _| {
                    if e.click_count == 2 && can_zoom {
                        drag.set(None);
                        window.zoom_window();
                    } else {
                        drag.set(Some(e.position));
                    }
                })
                .on_mouse_move(move |e, window, _| {
                    if e.pressed_button != Some(MouseButton::Left) {
                        motion.set(None);
                        return;
                    }
                    if let Some(start) = motion.get()
                        && ((e.position.x - start.x).abs() >= px(3.)
                            || (e.position.y - start.y).abs() >= px(3.))
                    {
                        motion.set(None);
                        window.start_window_move();
                    }
                })
            });
        let content_focus = content_focus.clone();
        div()
            .w_full()
            .h(px(42.))
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .border_b_1()
            .border_color(rgb(theme.border))
            .bg(rgb(theme.panel))
            .text_color(rgb(theme.text))
            .on_key_down(move |event, window, cx| {
                match event.keystroke.key.as_str() {
                    "tab" if event.keystroke.modifiers.shift => window.focus_prev(cx),
                    "tab" => window.focus_next(cx),
                    "escape" | "f6" => content_focus.focus(window, cx),
                    _ => return,
                }
                cx.stop_propagation();
            })
            .child(titlebar)
            .child(actions)
            .children(visible.into_iter().map(|control| {
                let (label, focus) = match control {
                    Control::Minimize => ("Minimize", &self.minimize),
                    Control::Maximize => ("Maximize", &self.maximize),
                    Control::Restore => ("Restore", &self.maximize),
                    Control::Close => ("Close", &self.close),
                };
                let close = self.close_handler.clone();
                button_base(label, focus, theme, move |window, cx| match control {
                    Control::Minimize => window.minimize_window(),
                    Control::Maximize | Control::Restore => window.zoom_window(),
                    Control::Close => close(window, cx),
                })
                .size(px(32.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(symbol(control, theme))
            }))
    }
}

fn symbol(control: Control, theme: Theme) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let at = |x, y| bounds.origin + point(px(x), px(y));
            let mut path = PathBuilder::stroke(px(1.5));
            let mut line = |coords: &[(f32, f32)]| {
                path.move_to(at(coords[0].0, coords[0].1));
                for &(x, y) in &coords[1..] {
                    path.line_to(at(x, y));
                }
            };
            match control {
                Control::Minimize => line(&[(2., 10.), (12., 10.)]),
                Control::Maximize => line(&[(2., 2.), (12., 2.), (12., 12.), (2., 12.), (2., 2.)]),
                Control::Restore => {
                    line(&[(4., 4.), (4., 1.), (13., 1.), (13., 10.), (10., 10.)]);
                    line(&[(1., 4.), (10., 4.), (10., 13.), (1., 13.), (1., 4.)]);
                }
                Control::Close => {
                    line(&[(2., 2.), (12., 12.)]);
                    line(&[(12., 2.), (2., 12.)]);
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(theme.text));
            }
        },
    )
    .size(px(14.))
}

#[cfg(test)]
mod tests {
    use super::{Control::*, controls};
    use gpui::{Decorations, WindowControls};
    #[test]
    fn negotiate_chrome() {
        let client = Decorations::Client {
            tiling: Default::default(),
        };
        let caps = WindowControls::default();
        assert_eq!(
            controls(client, caps, false, false),
            [Minimize, Maximize, Close]
        );
        assert_eq!(
            controls(client, caps, true, false),
            [Minimize, Restore, Close]
        );
        for (minimize, maximize, expected) in [
            (false, false, vec![Close]),
            (true, false, vec![Minimize, Close]),
            (false, true, vec![Maximize, Close]),
        ] {
            assert_eq!(
                controls(
                    client,
                    WindowControls {
                        minimize,
                        maximize,
                        ..caps
                    },
                    false,
                    false
                ),
                expected
            );
        }
        assert!(controls(Decorations::Server, caps, false, false).is_empty());
        assert!(controls(client, caps, false, true).is_empty());
    }
}
