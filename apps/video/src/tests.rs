use super::{Phase, Video};
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn until(view: &Entity<Video>, cx: &mut VisualTestContext, check: impl Fn(&Video) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        // Pump native notifications on the GPUI test scheduler's owning thread.
        cx.update(|w, cx| {
            view.update(cx, |video, cx| {
                if video.player.events().try_recv().is_ok() {
                    video.receive(w, cx);
                }
            })
        });
        cx.run_until_parked();
        let (ok, error) = cx.update(|_, cx| {
            let video = view.read(cx);
            (check(video), video.status.error.clone())
        });
        if ok {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for playback, error {error:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[gpui::test]
fn open_seek_pause_and_replay(cx: &mut TestAppContext) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (view, cx) = cx.add_window_view(|w, cx| {
        let mut video = Video::new(w, cx);
        video.open(root.join("../../tests/fixtures/Silent passage.webm"));
        video
    });
    until(&view, cx, |v| {
        v.status.phase == Phase::Playing && v.frame.is_some() && v.status.position > 50
    });
    cx.simulate_keystrokes("space");
    until(&view, cx, |v| v.status.phase == Phase::Paused);
    cx.simulate_keystrokes("f11");
    cx.update(|w, cx| {
        assert!(w.is_fullscreen());
        assert!(view.read(cx).screen.is_focused(w));
    });
    cx.simulate_keystrokes("f6");
    cx.update(|w, cx| assert!(view.read(cx).fullscreen_button.is_focused(w)));
    cx.simulate_keystrokes("escape");
    cx.update(|w, _| assert!(!w.is_fullscreen()));
    cx.simulate_keystrokes("home");
    until(&view, cx, |v| v.status.position < 10);
    cx.simulate_keystrokes("right");
    until(&view, cx, |v| v.status.position == 5000);
    cx.update(|w, cx| {
        let v = view.read(cx);
        assert!(v.screen.is_focused(w));
        assert_eq!(v.dimensions, (400, 224));
        assert_eq!(v.status.duration, Some(8000));
    });
    // Resume near the end. EOS keeps the final frame and Replay starts at zero.
    cx.simulate_keystrokes("end space");
    until(&view, cx, |v| v.status.phase == Phase::Ended);
    cx.simulate_keystrokes("space");
    until(&view, cx, |v| {
        v.status.phase == Phase::Playing && v.status.position < 1500
    });
    cx.update(|_, cx| view.update(cx, |v, _| v.open(root.join("Cargo.toml"))));
    until(&view, cx, |v| v.status.error.is_some());
    cx.update(|_, cx| assert_eq!(view.read(cx).name(), "Silent passage.webm"));
}
