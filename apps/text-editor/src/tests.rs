use super::Editor;
use gpui::{Focusable, TestAppContext};
use std::fs;
use std::path::Path;

#[gpui::test]
fn edit_find_dismiss_close_and_save(cx: &mut TestAppContext) {
    let temp =
        tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")).unwrap();
    let path = temp.path().join("notes.txt");
    fs::write(&path, "東京\r\n").unwrap();
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(|w, cx| {
        let mut editor = Editor::new(w, cx);
        editor.open(path.clone(), w, cx);
        editor
    });
    cx.simulate_keystrokes("ctrl-end h e l l o enter w o r l d");
    cx.update(|_, cx| {
        let editor = view.read(cx);
        assert_eq!(editor.body.read(cx).text(), "東京\r\nhello\r\nworld");
        assert!(editor.dirty(cx));
    });
    cx.simulate_keystrokes("ctrl-f w o r l d enter");
    cx.update(|w, cx| {
        let editor = view.read(cx);
        let field = editor.body.read(cx);
        assert_eq!(&field.text()[field.selection()], "world");
        assert!(editor.query.read(cx).focus_handle(cx).is_focused(w));
    });
    cx.simulate_keystrokes("escape");
    assert!(!cx.simulate_close());
    cx.update(|w, cx| {
        assert!(view.read(cx).confirmation.as_ref().unwrap().buttons[0].is_focused(w))
    });
    cx.simulate_keystrokes("shift-tab tab escape");
    cx.update(|w, cx| assert!(view.read(cx).body.read(cx).focus_handle(cx).is_focused(w)));
    assert_eq!(fs::read(&path).unwrap(), "東京\r\n".as_bytes());
    cx.simulate_keystrokes("ctrl-s");
    assert_eq!(
        fs::read(&path).unwrap(),
        "東京\r\nhello\r\nworld".as_bytes()
    );
    assert!(cx.simulate_close());
}

#[gpui::test]
fn untitled_save_cancel_and_retry(cx: &mut TestAppContext) {
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(Editor::new);
    cx.simulate_keystrokes("a enter tab b ctrl-z ctrl-shift-z ctrl-s");
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert!(view.read_with(cx, |e, cx| e.dirty(cx)));
    let temp =
        tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")).unwrap();
    let dest = temp.path().join("new.txt");
    cx.simulate_keystrokes("ctrl-s");
    cx.simulate_new_path_selection(|_| Some(dest.clone()));
    cx.run_until_parked();
    assert_eq!(fs::read(dest).unwrap(), b"a\n\tb");
    assert!(cx.simulate_close());
}

#[gpui::test]
fn failed_save_from_close_keeps_edits(cx: &mut TestAppContext) {
    let temp =
        tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")).unwrap();
    let path = temp.path().join("shared.txt");
    fs::write(&path, "first").unwrap();
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(|w, cx| {
        let mut editor = Editor::new(w, cx);
        editor.open(path.clone(), w, cx);
        editor
    });
    cx.simulate_keystrokes("end space e d i t");
    fs::write(&path, "external").unwrap();
    cx.simulate_keystrokes("ctrl-w shift-tab");
    cx.simulate_keystrokes("enter");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let editor = view.read(cx);
        assert!(editor.error.as_deref().unwrap().contains("changed on disk"));
        assert_eq!(editor.body.read(cx).text(), "first edit");
        assert!(editor.body.read(cx).focus_handle(cx).is_focused(w));
    });
    assert_eq!(fs::read(path).unwrap(), b"external");
    assert!(!cx.simulate_close());
}

#[gpui::test]
fn composition_offsets_preserve_selection_and_cancel(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;
    let (editor, cx) = cx.add_window_view(Editor::new);
    cx.update(|window, cx| {
        let field = editor.read(cx).body.clone();
        field.update(cx, |field, cx| {
            field.set_document("a😀e\u{301}\r\nz".into(), cx);
            field.replace_and_mark_text_in_range(Some(1..3), "候😀", Some(1..3), window, cx);
            assert_eq!(field.text(), "a候😀e\u{301}\r\nz");
            assert_eq!(field.marked_text_range(window, cx), Some(1..4));
            let selected = field.selected_text_range(false, window, cx).unwrap();
            assert_eq!(selected.range, 2..4);
            field.replace_text_in_range(None, "確", window, cx);
            assert_eq!(field.text(), "a確e\u{301}\r\nz");
            assert_eq!(field.marked_text_range(window, cx), None);
            field.replace_and_mark_text_in_range(None, "仮", None, window, cx);
            field.replace_and_mark_text_in_range(None, "", None, window, cx);
            assert_eq!(field.text(), "a確e\u{301}\r\nz");
            assert_eq!(field.marked_text_range(window, cx), None);
        });
    });
}
