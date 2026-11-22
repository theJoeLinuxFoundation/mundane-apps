use super::{Music, Phase, Playback};
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/music")
        .join(name)
}
fn until(v: &Entity<Music>, cx: &mut VisualTestContext, ok: impl Fn(&Music) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        cx.run_until_parked();
        let ready = cx.update(|w, cx| {
            v.update(cx, |m, cx| {
                if m.player.events().try_recv().is_ok() {
                    m.receive(w, cx)
                }
                assert!(m.status.error.is_none(), "{:?}", m.status.error);
                ok(m)
            })
        });
        if ready {
            return;
        }
        assert!(Instant::now() < deadline, "timed out on Music state");
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[gpui::test]
fn reorder_skip_pause_and_follow_eos(cx: &mut TestAppContext) {
    let (v, cx) = cx.add_window_view(|w, cx| {
        let mut m = Music::new(Playback::silent_for_test(), w, cx);
        m.append(
            ["01.flac", "03.ogg", "02.opus", "Field recording.wav"]
                .map(file)
                .into(),
            w,
            cx,
        );
        m
    });
    until(&v, cx, |m| {
        m.status.phase == Phase::Playing && m.queue.tracks.iter().all(|t| t.info.duration.is_some())
    });
    cx.update(|_, cx| {
        let tracks = &v.read(cx).queue.tracks;
        assert_eq!(
            tracks.iter().map(|t| t.title()).collect::<Vec<_>>(),
            [
                "Ebb Tide",
                "North Channel",
                "Harbour Lights",
                "Field recording.wav"
            ]
        );
        assert_eq!(tracks[0].info.artist.as_deref(), Some("Soundings"));
        assert_eq!(tracks[1].info.album.as_deref(), Some("Coastal Sessions"));
    });
    cx.simulate_keystrokes("down down ctrl-up n");
    until(&v, cx, |m| {
        m.queue.current == Some(3) && m.status.phase == Phase::Playing
    });
    cx.simulate_keystrokes("space");
    until(&v, cx, |m| m.status.phase == Phase::Paused);
    cx.update(|w, cx| {
        let m = v.read(cx);
        assert!(m.list.is_focused(w));
        assert_eq!(m.queue.selected, m.queue.current);
        assert_eq!(m.queue.tracks[1].id, 3)
    });
    until(&v, cx, |m| {
        m.peaks
            .as_ref()
            .is_some_and(|p| p.iter().any(|x| *x > 0.01))
    });
    cx.simulate_keystrokes("]");
    until(&v, cx, |m| {
        m.status.speed.factor() == 1.25 && m.status.phase == Phase::Paused
    });
    cx.simulate_keystrokes("right");
    until(&v, cx, |m| {
        m.status.position >= 5000 && m.status.speed.factor() == 1.25
    });
    cx.simulate_keystrokes("end space");
    until(&v, cx, |m| {
        m.queue.current == Some(2) && m.status.phase == Phase::Playing
    });
    cx.simulate_keystrokes("n");
    until(&v, cx, |m| {
        m.queue.current == Some(4) && m.status.phase == Phase::Ended
    });
    cx.simulate_keystrokes("space");
    until(&v, cx, |m| m.status.phase == Phase::Playing);
}
