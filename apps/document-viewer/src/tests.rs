use super::{Request, Viewer, ZOOMS};
use gpui::{Focusable, TestAppContext};
use std::path::Path;

#[gpui::test]
fn navigate_find_and_zoom(cx: &mut TestAppContext) {
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(|w, cx| {
        let mut viewer = Viewer::new(w, cx);
        viewer.request(
            Request::Open(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/Coastal survey.pdf"),
            ),
            w,
            cx,
        );
        viewer
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-l 2 enter");
    cx.run_until_parked();
    cx.update(|w, cx| {
        let v = view.read(cx);
        assert_eq!(v.state.as_ref().unwrap().page, 2);
        assert!(v.content.is_focused(w));
    });
    cx.simulate_keystrokes("ctrl-f h i g h enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-=");
    cx.run_until_parked();
    cx.update(|w, cx| {
        let v = view.read(cx);
        let s = v.state.as_ref().unwrap();
        assert_eq!(ZOOMS[s.zoom], 1.25);
        assert_eq!(s.matches.hits.len(), 1);
        assert!(!v.sheet.as_ref().unwrap().highlights.is_empty());
        assert!(v.query.read(cx).focus_handle(cx).is_focused(w));
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.simulate_keystrokes("pageup");
    cx.run_until_parked();
    cx.update(|w, cx| {
        let v = view.read(cx);
        assert_eq!(v.state.as_ref().unwrap().page, 1);
        assert!(!v.finding);
        assert!(v.content.is_focused(w));
    });
    cx.simulate_keystrokes("ctrl-f c h a n n e l enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("shift-f3");
    cx.run_until_parked();
    cx.update(|_, cx| {
        let state = view.read(cx).state.as_ref().unwrap();
        assert_eq!(state.page, 2);
        assert_eq!(state.matches.index, Some(1));
    });
    cx.update(|w, cx| {
        view.update(cx, |v, cx| {
            v.request(
                Request::Open(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")),
                w,
                cx,
            )
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let viewer = view.read(cx);
        assert!(viewer.error.as_ref().unwrap().contains("Cannot read PDF"));
        assert_eq!(viewer.state.as_ref().unwrap().page, 2);
        assert!(viewer.sheet.as_ref().unwrap().text.contains("Tidal range"));
    });
}
