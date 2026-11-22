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
        assert_eq!(editor.page().body.read(cx).text(), "東京\r\nhello\r\nworld");
        assert!(editor.dirty(cx));
    });
    cx.simulate_keystrokes("ctrl-f w o r l d enter");
    cx.update(|w, cx| {
        let editor = view.read(cx);
        let field = editor.page().body.read(cx);
        assert_eq!(&field.text()[field.selection()], "world");
        assert!(editor.query.read(cx).focus_handle(cx).is_focused(w));
    });
    cx.simulate_keystrokes("escape");
    assert!(!cx.simulate_close());
    cx.update(|w, cx| {
        assert!(view.read(cx).confirmation.as_ref().unwrap().buttons[0].is_focused(w))
    });
    cx.simulate_keystrokes("shift-tab tab escape");
    cx.update(|w, cx| {
        assert!(
            view.read(cx)
                .page()
                .body
                .read(cx)
                .focus_handle(cx)
                .is_focused(w)
        )
    });
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
        assert_eq!(editor.page().body.read(cx).text(), "first edit");
        assert!(editor.page().body.read(cx).focus_handle(cx).is_focused(w));
    });
    assert_eq!(fs::read(path).unwrap(), b"external");
    assert!(!cx.simulate_close());
}

#[gpui::test]
fn composition_offsets_preserve_selection_and_cancel(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;
    let (editor, cx) = cx.add_window_view(Editor::new);
    cx.update(|window, cx| {
        let field = editor.read(cx).page().body.clone();
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

#[gpui::test]
fn drafts_keep_their_edits_and_undo_history_when_switching(cx: &mut TestAppContext) {
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(Editor::new);
    cx.simulate_keystrokes("a b ctrl-n c ctrl-pageup ctrl-z");
    cx.update(|_, cx| {
        let e = view.read(cx);
        assert_eq!((e.pages.len(), e.active), (2, 0));
        assert_eq!(e.page().body.read(cx).text(), "a");
        assert_eq!(e.pages[1].body.read(cx).text(), "c");
    });
    // Closing the window must review the background draft even with a clean tab active.
    cx.simulate_keystrokes("ctrl-z");
    assert!(!cx.simulate_close());
    cx.update(|_, cx| {
        let e = view.read(cx);
        assert_eq!(e.active, 1);
        assert!(e.confirmation.is_some());
    });
    cx.simulate_keystrokes("escape ctrl-w");
    cx.update(|w, cx| view.update(cx, |e, cx| e.command(super::Command::Discard, w, cx)));
    assert_eq!(view.read_with(cx, |e, _| e.pages.len()), 1);
    assert!(cx.simulate_close());
}

#[gpui::test]
fn open_keeps_a_dirty_draft_and_a_reopened_file_keeps_its_caret(cx: &mut TestAppContext) {
    cx.update(mundane_ui::bind_text_keys);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("example.txt");
    fs::write(&path, "first\r\n東京\r\nlast\r\n").unwrap();
    let (view, cx) = cx.add_window_view(Editor::new);
    cx.simulate_keystrokes("a");
    cx.update(|w, cx| view.update(cx, |e, cx| e.open(path.clone(), w, cx)));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |e, _| (e.pages.len(), e.active)), (2, 1));
    cx.simulate_keystrokes("ctrl-i 9 enter");
    assert!(view.read_with(cx, |e, _| e.going
        && e.error.as_deref() == Some("Enter a line from 1 to 4.")));
    cx.simulate_keystrokes("ctrl-a 3 enter");
    cx.update(|w, cx| {
        let e = view.read(cx);
        let f = e.page().body.read(cx);
        assert_eq!(f.line_column(), (3, 1));
        assert_eq!(f.selection().start, "first\r\n東京\r\n".len());
        assert!(!e.going && f.focus_handle(cx).is_focused(w));
    });
    cx.simulate_keystrokes("ctrl-pageup");
    cx.update(|w, cx| view.update(cx, |e, cx| e.open(path, w, cx)));
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |e, cx| (
            e.pages.len(),
            e.active,
            e.page().body.read(cx).line_column()
        )),
        (2, 1, (3, 1))
    );
    cx.simulate_keystrokes("ctrl-w");
    assert_eq!(
        view.read_with(cx, |e, cx| e.page().body.read(cx).text().to_owned()),
        "a"
    );
    assert!(!cx.simulate_close());
}

#[gpui::test]
fn replace_all_is_one_undo_and_preserves_unmatched_line_endings(cx: &mut TestAppContext) {
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(Editor::new);
    cx.update(|_, cx| {
        view.read(cx)
            .page()
            .body
            .clone()
            .update(cx, |f, cx| f.set_document("猫 x\r\nx\nz".into(), cx))
    });
    cx.simulate_keystrokes("ctrl-h x tab y");
    cx.update(|w, cx| view.update(cx, |e, cx| e.command(super::Command::ReplaceAll, w, cx)));
    assert_eq!(
        view.read_with(cx, |e, cx| e.page().body.read(cx).snapshot())
            .as_str(),
        "猫 y\r\ny\nz"
    );
    cx.simulate_keystrokes("escape ctrl-z");
    assert_eq!(
        view.read_with(cx, |e, cx| e.page().body.read(cx).snapshot())
            .as_str(),
        "猫 x\r\nx\nz"
    );
}

#[test]
fn recover_only_abandoned_drafts_and_preserve_external_change_guard() {
    use super::{document::Document, state::*};
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("source.py");
    fs::write(&file, b"\xef\xbb\xbfpass\r\n").unwrap();
    let doc = Document::open(file.clone()).unwrap();
    let draft = Draft::new(&doc, "# edited\r\n").unwrap();
    let slot = Slot::create(dir.path()).unwrap();
    let generation = slot.next();
    slot.write(generation, &draft).unwrap();
    assert!(recover(dir.path()).unwrap().is_empty());
    drop(slot);
    let (slot, saved) = recover(dir.path()).unwrap().pop().unwrap();
    let (base, text) = saved.into_document().unwrap();
    assert_eq!(text, "# edited\r\n");
    assert!(base.bom);
    fs::write(&file, "changed outside").unwrap();
    assert!(base.save(file.clone(), text.into()).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "changed outside");
    let stale = slot.next();
    slot.clear().unwrap();
    slot.write(stale, &draft).unwrap();
    drop(slot);
    assert!(recover(dir.path()).unwrap().is_empty());
    remember(dir.path(), &file).unwrap();
    let next = dir.path().join("notes");
    remember(dir.path(), &next).unwrap();
    remember(dir.path(), &file).unwrap();
    assert_eq!(recent(dir.path()).unwrap(), vec![file, next]);
}

#[test]
fn supported_grammars_highlight_unicode_and_multiline_strings() {
    for (lang, text) in [
        ("Python", "name = '''東京\nquay'''"),
        ("Rust", "let name = r#\"波\"#;"),
        ("JSON", "{\"name\": \"東京\"}"),
    ] {
        let spans = super::syntax::spans(lang, text);
        assert!(
            spans.iter().any(|(r, role)| *role == super::syntax::STRING
                && text[r.clone()].contains(['東', '波']))
        );
        assert!(spans.windows(2).all(|s| s[0].0.end <= s[1].0.start));
    }
}
