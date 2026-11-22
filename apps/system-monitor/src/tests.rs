use super::{Column, INTERVAL, Monitor};
use gpui::{KeyUpEvent, Keystroke, TestAppContext};
use mundane_services::processes::{Counters, ProcessId, ProcessService, Sample};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Instant,
};

struct Fake {
    start: Instant,
    step: Mutex<u32>,
    signals: Mutex<Vec<ProcessId>>,
}

impl ProcessService for Fake {
    fn sample(&self) -> io::Result<Sample> {
        let mut step = self.step.lock().unwrap();
        let n = *step;
        *step += 1;
        let processes = [(10, "alpha", 5), (20, "worker", 140)]
            .into_iter()
            .map(|(pid, name, rate)| Counters {
                id: ProcessId {
                    pid,
                    start_ticks: if pid == 20 && n >= 2 { 999 } else { 1 },
                },
                name: name.into(),
                cpu_ticks: n as u64 * rate,
                resident_bytes: 8192 * pid as u64,
            })
            .collect();
        Ok(Sample {
            at: self.start + INTERVAL * n,
            ticks_per_second: 100,
            processes,
        })
    }
    fn terminate(&self, id: ProcessId) -> io::Result<()> {
        self.signals.lock().unwrap().push(id);
        Ok(())
    }
}

#[gpui::test]
fn sort_select_and_confirm_the_same_process(cx: &mut TestAppContext) {
    let fake = Arc::new(Fake {
        start: Instant::now(),
        step: Mutex::new(0),
        signals: Mutex::new(Vec::new()),
    });
    let clock = cx.executor().clone();
    let (view, cx) = cx.add_window_view(|w, cx| Monitor::new(fake.clone(), w, cx));
    cx.run_until_parked();
    clock.advance_clock(INTERVAL);
    cx.run_until_parked();
    cx.simulate_keystrokes("c home delete");
    let id = ProcessId {
        pid: 20,
        start_ticks: 1,
    };
    cx.update(|window, cx| {
        let m = view.read(cx);
        assert!(m.column == Column::Cpu && m.descending);
        assert_eq!(m.rows[0].cpu_percent, Some(70.));
        assert_eq!(m.selected, Some(id));
        assert!(m.confirm.as_ref().unwrap().cancel.is_focused(window));
    });
    cx.simulate_keystrokes("escape");
    cx.update(|w, cx| assert!(view.read(cx).list_focus.is_focused(w)));
    assert!(fake.signals.lock().unwrap().is_empty());
    cx.simulate_keystrokes("delete shift-tab");
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    assert_eq!(*fake.signals.lock().unwrap(), [id]);
    cx.simulate_keystrokes("delete tab");
    clock.advance_clock(INTERVAL);
    cx.run_until_parked();
    cx.update(|w, cx| {
        let monitor = view.read(cx);
        assert!(monitor.selected.is_none());
        assert!(monitor.confirm.as_ref().unwrap().cancel.is_focused(w));
        assert_eq!(monitor.rows.last().unwrap().cpu_percent, None);
    });
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let monitor = view.read(cx);
        assert!(monitor.confirm.is_none());
        assert!(monitor.list_focus.is_focused(w));
    });
    assert_eq!(*fake.signals.lock().unwrap(), [id]);
}
