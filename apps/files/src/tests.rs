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
        Some(Prompt::Conflict(_))
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
