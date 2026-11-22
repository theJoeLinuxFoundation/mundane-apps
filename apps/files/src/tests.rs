use super::{Files, dialogs::Prompt};
use gpui::TestAppContext;
use std::{fs, path::Path};

#[gpui::test]
fn browse_rename_and_cancel_copy_conflict(cx: &mut TestAppContext) {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let temp = tempfile::tempdir_in(parent).unwrap();
    let folder = temp.path().join("Documents");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("draft"), "one").unwrap();
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(|window, cx| Files::new(temp.path().into(), window, cx));
    cx.simulate_keystrokes("enter");
    assert_eq!(view.read_with(cx, |f, _| f.path.clone()), folder);
    cx.simulate_keystrokes("f2 n o t e s enter");
    assert_eq!(fs::read(folder.join("notes")).unwrap(), b"one");
    assert!(!folder.join("draft").exists());
    cx.simulate_keystrokes("ctrl-c ctrl-v");
    assert!(view.read_with(cx, |f, _| matches!(
        f.dialog.as_ref().map(|d| &d.prompt),
        Some(Prompt::Conflict(..))
    )));
    cx.update(|window, cx| {
        assert!(
            view.read(cx)
                .dialog
                .as_ref()
                .unwrap()
                .cancel
                .is_focused(window)
        )
    });
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| assert!(view.read(cx).list_focus.is_focused(window)));
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
    cx.simulate_keystrokes("ctrl-v");
    cx.simulate_keystrokes("tab");
    // TestAppContext's keystroke simulator dispatches key-down only for buttons.
    cx.simulate_keystrokes("enter");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    assert_eq!(fs::read(folder.join("notes (copy)")).unwrap(), b"one");
}

#[gpui::test]
fn search_nested_results_and_return_to_folder(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let sub = temp.path().join("Archive");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("Notes.txt"), "saved").unwrap();
    cx.update(mundane_ui::bind_text_keys);
    let (view, ui) = cx.add_window_view(|w, cx| Files::new(temp.path().into(), w, cx));
    ui.simulate_keystrokes("ctrl-f n o t e enter");
    ui.run_until_parked();
    view.read_with(ui, |v, _| {
        assert_eq!(v.nav.query.as_deref(), Some("note"));
        assert_eq!(v.selected().unwrap().path, sub.join("Notes.txt"));
    });
    ui.simulate_keystrokes("escape");
    ui.run_until_parked();
    view.read_with(ui, |v, _| {
        assert!(v.nav.query.is_none());
        assert_eq!(v.selected().unwrap().path, sub)
    });
}

#[gpui::test]
fn tabs_keep_search_selection_and_see_sibling_changes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("Archive");
    fs::create_dir(&folder).unwrap();
    let note = folder.join("Notes");
    fs::write(&note, "draft").unwrap();
    cx.update(mundane_ui::bind_text_keys);
    let (view, ui) = cx.add_window_view(|w, cx| Files::new(dir.path().into(), w, cx));
    ui.simulate_keystrokes("ctrl-f n o t e enter");
    ui.run_until_parked();
    ui.simulate_keystrokes("ctrl-t");
    ui.run_until_parked();
    ui.simulate_keystrokes("enter");
    ui.run_until_parked();
    view.read_with(ui, |v, _| {
        assert_eq!((v.tabs.len(), v.active), (2, 1));
        assert_eq!(v.path, folder);
        assert!(v.nav.query.is_none());
    });
    fs::write(folder.join("Notes copy"), "another").unwrap();
    ui.simulate_keystrokes("ctrl-pageup");
    ui.run_until_parked();
    view.read_with(ui, |v, _| {
        assert_eq!(v.nav.query.as_deref(), Some("note"));
        assert_eq!(v.entries.len(), 2);
        assert_eq!(v.selected().unwrap().path, note);
    });
    ui.simulate_keystrokes("ctrl-w");
    ui.run_until_parked();
    view.read_with(ui, |v, _| {
        assert_eq!(v.tabs.len(), 1);
        assert_eq!(v.path, folder);
    });
}
