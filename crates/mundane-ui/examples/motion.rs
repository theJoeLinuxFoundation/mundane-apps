//! Wayland test fixture for GPUI's handling of non-essential animation.
use gpui::{prelude::*, *};
use mundane_ui::{Theme, browser_window};
use std::time::Duration;

struct Motion;
impl Render for Motion {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_4()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .child(if cx.reduce_motion() {
                "Reduced motion"
            } else {
                "Standard motion"
            })
            .child(
                div()
                    .relative()
                    .w(px(240.))
                    .h(px(12.))
                    .rounded_md()
                    .bg(rgb(theme.panel))
                    .child(
                        div()
                            .absolute()
                            .size(px(12.))
                            .rounded_full()
                            .bg(rgb(theme.focus))
                            .with_animation(
                                "marker",
                                Animation::new(Duration::from_secs(2)).repeat(),
                                |marker, progress| marker.left(px(228. * progress)),
                            ),
                    ),
            )
    }
}

fn main() {
    browser_window(
        "Motion preview",
        "org.joelinux.mundane.MotionFixture",
        |_, _| Motion,
    )
}
