use super::{Answer, Job, Packages, Page};
use gpui::{KeyUpEvent, Keystroke, TestAppContext, VisualTestContext};
use mundane_services::packages::{Dave, Operation, Request};
use std::{fs, os::unix::fs::symlink, path::Path};

fn fixture() -> (tempfile::TempDir, Dave) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir_in(root.join("target")).unwrap();
    for name in ["pkexec", "dave"] {
        symlink(
            root.join("tests/packages/fixture.py"),
            dir.path().join(name),
        )
        .unwrap()
    }
    let dave = Dave::with_programs(dir.path().join("dave"), dir.path().join("pkexec"));
    (dir, dave)
}
fn activate(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked()
}
#[gpui::test]
fn review_dependencies_cancel_and_confirm_changed_plan(cx: &mut TestAppContext) {
    let (dir, dave) = fixture();
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(|w, cx| Packages::new(dave, w, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-f c h a r t enter");
    cx.run_until_parked();
    assert!(view.read_with(
        cx,
        |v, _| matches!(&v.page,Page::Results(_,s) if s.packages[0].name=="chartroom")
    ));
    activate(cx);
    assert!(view.read_with(
        cx,
        |v, _| matches!(&v.page,Page::Details(i) if i.depends.contains(&"libtide".into()))
    ));
    activate(cx);
    cx.update(|w, cx| assert!(view.read(cx).cancel.is_focused(w)));
    cx.simulate_keystrokes("escape");
    assert!(
        !fs::read_to_string(dir.path().join("calls.jsonl"))
            .unwrap()
            .contains("pkexec")
    );
    activate(cx);
    let old = view.read_with(cx, |v, _| v.review.as_ref().unwrap().plan().digest.clone());
    fs::write(dir.path().join("mode"), "moved").unwrap();
    cx.simulate_keystrokes("tab");
    activate(cx);
    cx.update(|w, cx| {
        let v = view.read(cx);
        assert!(v.cancel.is_focused(w) && v.review_message.contains("plan changed"));
        assert_ne!(v.review.as_ref().unwrap().plan().digest, old);
        assert_eq!(v.review.as_ref().unwrap().plan().install[0].version, "2.1");
    });
    assert!(!dir.path().join("applied").exists());
    cx.simulate_keystrokes("tab");
    activate(cx);
    assert_eq!(
        fs::read_to_string(dir.path().join("applied")).unwrap(),
        "install\n"
    );
    assert!(view.read_with(cx, |v, _| v.review.is_none()
        && v.notice.contains("Rollback point 12")
        && matches!(&v.page,Page::Installed(l) if l.iter().any(|p|p.name=="chartroom"))));
}
#[test]
fn authorization_outcomes_removal_and_preparation() {
    let (dir, dave) = fixture();
    let install = || {
        dave.preview(Request::package(Operation::Install, "chartroom").unwrap())
            .unwrap()
    };
    for (mode, message) in [
        ("dismiss", "dismissed"),
        ("denied", "denied"),
        ("refused", "dave refused"),
    ] {
        fs::write(dir.path().join("mode"), mode).unwrap();
        assert!(
            matches!(Job::Apply(install()).execute(&dave).unwrap(),Answer::Notice(s) if s.contains(message))
        );
        assert!(!dir.path().join("applied").exists());
    }
    fs::write(dir.path().join("mode"), "success").unwrap();
    assert!(matches!(
        Job::Apply(install()).execute(&dave).unwrap(),
        Answer::Changed(_, Ok(Page::Installed(_)))
    ));
    assert!(install().plan().digest.is_none());
    assert!(dave.apply(install()).is_err());
    let remove = dave
        .preview(Request::package(Operation::Remove, "chartroom").unwrap())
        .unwrap();
    assert_eq!(remove.plan().remove[0].name, "chartroom");
    Job::Apply(remove).execute(&dave).unwrap();
    let upgrade = dave.preview(Request::upgrade()).unwrap();
    assert!(
        matches!(Job::Apply(upgrade).execute(&dave).unwrap(),Answer::Changed(_,Ok(Page::Status(s))) if s.prepared_upgrade.is_some() && s.running==Some(12))
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("applied")).unwrap(),
        "install\nremove\nupgrade\n"
    );
}

#[test]
fn only_changed_and_older_dave_refresh_the_plan() {
    for (mode, extra_preview, needs_review, different_digest) in [
        ("moved", true, true, true),
        ("changed-same", true, true, false),
        ("legacy-moved", true, true, true),
        ("refused", true, false, false), // Legacy refusal and unchanged digest.
        ("null", false, false, false),
        ("not_allowed", false, false, false),
        ("needs_root", false, false, false),
        ("future", false, false, false),
        ("dismiss", false, false, false),
        ("denied", false, false, false),
    ] {
        let (dir, dave) = fixture();
        fs::write(dir.path().join("mode"), mode).unwrap();
        let request = Request::package(Operation::Install, "chartroom").unwrap();
        let preview = dave.preview(request).unwrap();
        let original_digest = preview.plan().digest.clone();
        let answer = Job::Apply(preview).execute(&dave).unwrap();
        match answer {
            Answer::Review(new, _) => {
                assert!(needs_review, "{mode}");
                assert_eq!(
                    new.plan().digest != original_digest,
                    different_digest,
                    "{mode}"
                );
            }
            Answer::Notice(_) => assert!(!needs_review, "{mode}"),
            _ => panic!("unexpected result for {mode}"),
        }
        let calls = fs::read_to_string(dir.path().join("calls.jsonl")).unwrap();
        assert_eq!(
            calls.matches("--dry-run").count(),
            1 + usize::from(extra_preview),
            "{mode}"
        );
        assert_eq!(calls.matches("pkexec").count(), 1, "{mode}");
        assert!(!dir.path().join("applied").exists(), "{mode}");
    }
}
