use super::*;
use mundane_ui::{ControlRole, Role, button, focus_ring, unavailable};

impl Viewer {
    fn tool(&self, cmd: Command, cx: &Context<Self>) -> Stateful<Div> {
        let (label, index) = match cmd {
            Command::Open => ("Open", 0),
            Command::Smaller => ("Zoom out", 1),
            Command::Larger => ("Zoom in", 2),
            Command::Fit => ("Fit", 3),
        };
        let view = cx.weak_entity();
        let control = button(label, &self.tools[index], self.theme, move |window, cx| {
            let _ = view.update(cx, |this, cx| this.command(cmd, window, cx));
        });
        let disabled = !self.enabled(cmd);
        unavailable(control, disabled).when(disabled, |el| el.text_color(rgb(self.theme.secondary)))
    }

    fn pan_by(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        if self.zoom > FIT {
            self.pan.x = (self.pan.x + x).clamp(-1., 1.);
            self.pan.y = (self.pan.y + y).clamp(-1., 1.);
            cx.notify();
        }
    }

    fn keys(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        match (key.key.as_str(), key.modifiers.control) {
            ("o", true) => self.command(Command::Open, window, cx),
            ("w", true) => window.remove_window(),
            ("+" | "=" | "plus" | "equal", _) => self.command(Command::Larger, window, cx),
            ("-" | "minus", _) => self.command(Command::Smaller, window, cx),
            ("0", _) | ("f", false) => self.command(Command::Fit, window, cx),
            ("left", false) => self.pan_by(-0.25, 0., cx),
            ("right", false) => self.pan_by(0.25, 0., cx),
            ("up", false) => self.pan_by(0., -0.25, cx),
            ("down", false) => self.pan_by(0., 0.25, cx),
            ("f6", false) => self.tools[0].focus(window, cx),
            ("escape", false) => self.image_focus.focus(window, cx),
            ("tab", false) if key.modifiers.shift => window.focus_prev(cx),
            ("tab", false) => window.focus_next(cx),
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    fn viewport(&self, cx: &Context<Self>) -> Stateful<Div> {
        let name = self
            .picture
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("Image area");
        let role = if self.picture.is_some() {
            Role::Image
        } else {
            Role::Group
        };
        let area = focus_ring(
            role.control("picture", name.to_owned()),
            &self.image_focus,
            self.theme,
        )
        .flex_1()
        .min_h_0()
        .p_3()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, cx| this.image_focus.focus(window, cx)),
        )
        .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
            let delta = event.delta.pixel_delta(px(20.));
            this.pan_by(-f32::from(delta.x) / 160., -f32::from(delta.y) / 160., cx);
        }));
        let Some(pic) = &self.picture else {
            return area.child("Open a PNG or JPEG image");
        };
        let pixels = pic.pixels.clone();
        let image_id = pixels.id;
        let view = cx.weak_entity();
        let (width, height) = (pic.width as f32, pic.height as f32);
        let zoom = ZOOM[self.zoom];
        let pan = self.pan;
        area.aria_description(format!("{} × {} pixels", pic.width, pic.height))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        let scale = (f32::from(bounds.size.width) / width)
                            .min(f32::from(bounds.size.height) / height)
                            * zoom;
                        let size = size(px(width * scale), px(height * scale));
                        let half_x = (bounds.size.width - size.width) * 0.5;
                        let half_y = (bounds.size.height - size.height) * 0.5;
                        let offset = point(
                            half_x + half_x.min(px(0.)) * pan.x,
                            half_y + half_y.min(px(0.)) * pan.y,
                        );
                        let destination = Bounds::new(bounds.origin + offset, size);
                        if let Err(err) = window.paint_image(
                            bounds,
                            destination,
                            Default::default(),
                            pixels,
                            0,
                            false,
                        ) {
                            window.defer(cx, move |window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    if this
                                        .picture
                                        .as_ref()
                                        .is_some_and(|p| p.pixels.id == image_id)
                                    {
                                        this.picture = None;
                                        window.set_window_title("Image Viewer");
                                        this.error =
                                            Some(format!("Could not display image. {err}"));
                                        cx.notify();
                                    }
                                });
                            });
                        }
                    },
                )
                .size_full(),
            )
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let zoom = if self.zoom == FIT {
            "Fit to window".to_owned()
        } else {
            format!("{:.0}% of fit", ZOOM[self.zoom] * 100.)
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .on_key_down(cx.listener(Self::keys))
            .child(self.header.render(
                "Image Viewer",
                self.tool(Command::Open, cx),
                &self.image_focus,
                theme,
                window,
            ))
            .child(self.viewport(cx))
            .when_some(self.picture.as_ref(), |el, pic| {
                let detail = format!("{} × {}  ·  {zoom}", pic.width, pic.height);
                el.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .bg(rgb(theme.panel))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(100.))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .truncate()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(pic.name.clone()),
                                )
                                .child(
                                    Role::Status
                                        .control("image-status", "Image status")
                                        .aria_description(detail.clone())
                                        .text_sm()
                                        .text_color(rgb(theme.secondary))
                                        .child(detail),
                                ),
                        )
                        .children(
                            [Command::Smaller, Command::Larger, Command::Fit]
                                .map(|cmd| self.tool(cmd, cx)),
                        ),
                )
            })
            .when_some(
                if self.busy {
                    Some("Opening image…".to_owned())
                } else {
                    self.error.clone()
                },
                |el, message| {
                    el.child(
                        Role::Status
                            .control("message", message.clone())
                            .px_3()
                            .py_1()
                            .child(message),
                    )
                },
            )
    }
}
