use super::Terminal;
use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line, Point},
    tty::{Options, Shell},
};
use gpui::{Focusable, Modifiers, MouseButton, TestAppContext, point, px};
use std::time::{Duration, Instant};

#[gpui::test]
fn type_select_copy_and_find(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(mundane_ui::bind_text_keys);
    // A non-interactive shell attached to the real PTY eliminates user rc files and prompt text.
    let options = Options {
        shell: Some(Shell::new("/bin/sh".into(), vec!["-s".into()])),
        ..Options::default()
    };
    let (view, cx) = cx.add_window_view(|window, cx| Terminal::new(options, window, cx).unwrap());
    cx.simulate_keystrokes("e c h o space g r i d enter");
    let deadline = Instant::now() + Duration::from_secs(8);
    let line = loop {
        let found = view.read_with(cx, |terminal, _| {
            let screen = terminal.tabs[terminal.active].session.screen.lock();
            (0..screen.screen_lines() as i32).find(|&row| {
                screen
                    .bounds_to_string(
                        Point::new(Line(row), Column(0)),
                        Point::new(Line(row), screen.last_column()),
                    )
                    .trim()
                    == "grid"
            })
        });
        if let Some(row) = found {
            break row;
        }
        assert!(Instant::now() < deadline, "shell output never reached grid");
        std::thread::sleep(Duration::from_millis(10));
        cx.run_until_parked();
    };
    let (start, end) = view.read_with(cx, |terminal, _| {
        let layout = terminal.layout;
        let cell = terminal.tabs[terminal.active].session.size();
        let y = (line as f32 + 0.5) * f32::from(cell.cell_height);
        let start = layout.bounds.origin + point(px(1.), px(y));
        (
            start,
            start + point(px(3.75 * f32::from(cell.cell_width)), px(0.)),
        )
    });
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    cx.simulate_keystrokes("ctrl-shift-c");
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("grid")
    );
    cx.simulate_keystrokes("ctrl-shift-f");
    cx.update(|window, cx| {
        assert!(
            view.read(cx)
                .find
                .as_ref()
                .unwrap()
                .field
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        )
    });
    cx.simulate_keystrokes("g r i d tab shift-tab enter");
    cx.update(|window, cx| {
        let view = view.read(cx);
        assert!(view.find.is_none());
        assert!(view.grid_focus.is_focused(window));
        assert_eq!(
            view.tabs[view.active]
                .session
                .screen
                .lock()
                .selection_to_string()
                .as_deref(),
            Some("grid")
        );
    });
}

#[gpui::test]
fn web_links_and_independent_shell_tabs(cx: &mut TestAppContext) {
    let app = cx.clone();
    cx.executor().allow_parking();
    let options = Options {
        shell: Some(Shell::new("/bin/sh".into(), vec!["-s".into()])),
        ..Options::default()
    };
    let (view, cx) = cx.add_window_view(|w, cx| Terminal::new(options, w, cx).unwrap());
    view.read_with(cx, |v, _| {
        v.tabs[0]
            .session
            .write(b"printf 'Visit https://example.org/docs.\\n'\n".to_vec())
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    let hit = loop {
        if let Some(pos) = view.read_with(cx, |v, _| {
            let tab = &v.tabs[0];
            let screen = tab.session.screen.lock();
            (0..screen.screen_lines() as i32).find_map(|row| {
                let point = Point::new(Line(row), Column(8));
                if super::links::at(&screen, point).as_deref() != Some("https://example.org/docs") {
                    return None;
                }
                let size = tab.session.size();
                Some(
                    v.layout.bounds.origin
                        + gpui::point(
                            px(8.5 * f32::from(size.cell_width)),
                            px((row as f32 + 0.5) * f32::from(size.cell_height)),
                        ),
                )
            })
        }) {
            break pos;
        }
        assert!(Instant::now() < deadline, "URL never reached the grid");
        std::thread::sleep(Duration::from_millis(10));
        cx.run_until_parked();
    };
    cx.simulate_mouse_down(
        hit,
        MouseButton::Left,
        Modifiers {
            control: true,
            ..Default::default()
        },
    );
    assert_eq!(
        app.opened_url().as_deref(),
        Some("https://example.org/docs")
    );
    cx.simulate_keystrokes("ctrl-shift-t");
    assert_eq!(view.read_with(cx, |v, _| (v.tabs.len(), v.active)), (2, 1));
    cx.simulate_keystrokes("ctrl-pageup");
    assert_eq!(view.read_with(cx, |v, _| v.active), 0);
    cx.simulate_keystrokes("ctrl-pagedown ctrl-shift-w");
    assert_eq!(view.read_with(cx, |v, _| (v.tabs.len(), v.active)), (1, 0));
}

#[test]
fn only_web_links_are_opened() {
    for value in [
        "file:///tmp/example",
        "javascript:alert(1)",
        "example.org",
        "https://example.org/ a",
        "https://",
    ] {
        assert!(super::links::web_url(value).is_none(), "{value}");
    }
}
