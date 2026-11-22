use super::{FIT, Viewer, ZOOM};
use gpui::{Point, TestAppContext, point};
use std::path::PathBuf;

#[gpui::test]
fn open_zoom_pan_fit_and_recover(cx: &mut TestAppContext) {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/North Point.jpg");
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut viewer = Viewer::new(window, cx);
        viewer.open(fixture, window, cx);
        viewer
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let viewer = view.read(cx);
        assert_eq!(viewer.picture.as_ref().unwrap().name, "North Point.jpg");
        assert!(viewer.image_focus.is_focused(window));
        assert_eq!(viewer.zoom, FIT);
    });
    cx.simulate_keystrokes("f6");
    cx.update(|window, cx| assert!(view.read(cx).tools[0].is_focused(window)));
    cx.simulate_keystrokes("escape ctrl-= right down");
    cx.update(|_, cx| {
        let viewer = view.read(cx);
        assert_eq!(ZOOM[viewer.zoom], 1.25);
        assert_eq!(viewer.pan, point(0.25, 0.25));
    });
    cx.simulate_keystrokes("tab tab tab");
    cx.update(|window, cx| assert!(view.read(cx).tools[3].is_focused(window)));
    cx.simulate_keystrokes("enter");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.update(|window, cx| {
        view.update(cx, |viewer, cx| {
            assert_eq!(viewer.zoom, FIT);
            assert_eq!(viewer.pan, Point::default());
            viewer.open(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
                window,
                cx,
            );
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let viewer = view.read(cx);
        assert!(
            viewer
                .error
                .as_ref()
                .unwrap()
                .contains("Could not open image")
        );
        assert_eq!(viewer.picture.as_ref().unwrap().name, "North Point.jpg");
    });
}
