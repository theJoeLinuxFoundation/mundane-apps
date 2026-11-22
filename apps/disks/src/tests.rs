use super::Disks;
use gpui::TestAppContext;
use mundane_services::disks::UDisks;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Fixture {
    temp: tempfile::TempDir,
    bus: Child,
    service: Child,
    client: UDisks,
}
impl Fixture {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = tempfile::tempdir_in(root.join("target")).unwrap();
        let dir = temp.path().canonicalize().unwrap();
        let config = dir.join("bus.conf");
        fs::write(&config,format!("<busconfig><type>session</type><listen>unix:path={}/bus</listen><policy context=\"default\"><allow own=\"*\"/><allow send_destination=\"*\"/><allow receive_sender=\"*\"/></policy></busconfig>",dir.display())).unwrap();
        let mut bus = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(bus.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(address.starts_with(&format!("unix:path={}", dir.display())));
        let address = address.trim().to_owned();
        let service = Command::new("python3")
            .arg(root.join("tests/disks/udisks.py"))
            .arg(&address)
            .arg(&dir)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .spawn()
            .unwrap();
        let mut fixture = Self {
            client: UDisks::at_address(address),
            bus,
            service,
            temp,
        };
        let start = Instant::now();
        while !dir.join("ready").exists() {
            assert!(fixture.service.try_wait().unwrap().is_none());
            assert!(start.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(10))
        }
        fixture
    }
    fn calls(&self) -> usize {
        fs::read_to_string(self.temp.path().join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for p in [&mut self.service, &mut self.bus] {
            let _ = p.kill();
            let _ = p.wait();
        }
    }
}

#[test]
fn fake_bus_denial_replacement_and_hot_removal() {
    let f = Fixture::new();
    let d = f.temp.path();
    let items = f.client.list().unwrap();
    let before = items
        .iter()
        .find(|n| n.label == "Survey")
        .unwrap()
        .volume
        .clone()
        .unwrap();
    fs::write(d.join("deny"), "").unwrap();
    assert!(
        f.client
            .change(&before)
            .unwrap_err()
            .to_string()
            .contains("Authorization denied")
    );
    assert!(!d.join("mounted").exists());
    fs::remove_file(d.join("deny")).unwrap();
    fs::write(d.join("replace"), "").unwrap();
    assert!(
        f.client
            .change(&before)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    assert_eq!(f.calls(), 1);
    fs::remove_file(d.join("replace")).unwrap();
    fs::write(d.join("remove-during"), "").unwrap();
    assert!(
        f.client
            .change(&before)
            .unwrap_err()
            .to_string()
            .contains("no longer available")
    );
    assert_eq!(f.calls(), 2);
    assert!(!f.client.list().unwrap().iter().any(|n| n.label == "Survey"));
    assert!(f.client.change(&before).is_err());
    assert_eq!(f.calls(), 2);
}

#[gpui::test]
fn select_mount_unmount_and_lose_selection(cx: &mut TestAppContext) {
    let f = Fixture::new();
    let timer = cx.executor().clone();
    let (v, ui) = cx.add_window_view(|w, cx| {
        let d = Disks::new(f.client.clone(), w, cx);
        d.watch(w, cx);
        d
    });
    ui.run_until_parked();
    ui.simulate_keystrokes("home down down ctrl-m");
    ui.run_until_parked();
    ui.update(|_, cx| {
        let d = v.read(cx);
        assert_eq!(
            d.selected().unwrap().mount_points,
            [Path::new("/run/media/joe/Survey")]
        );
        assert!(d.notice.starts_with("Mounted at"))
    });
    ui.simulate_keystrokes("ctrl-m");
    ui.run_until_parked();
    assert!(
        v.read_with(ui, |d, _| d.selected().unwrap().mount_points.is_empty()
            && d.notice == "Volume unmounted.")
    );
    fs::write(f.temp.path().join("removed"), "").unwrap();
    timer.advance_clock(Duration::from_secs(2));
    ui.run_until_parked();
    assert!(v.read_with(ui, |d, _| d.list.selected.is_none()));
    ui.simulate_keystrokes("ctrl-m");
    assert_eq!(f.calls(), 2);
}

#[test]
fn drive_operations_require_capability_stable_identity_and_idle_volumes() {
    use mundane_services::disks::Operation;
    let f = Fixture::new();
    let d = f.temp.path();
    fs::write(d.join("encrypted"), "").unwrap();
    let items = f.client.list().unwrap();
    let power = items[0]
        .actions
        .iter()
        .find(|a| a.operation == Operation::PowerOff)
        .unwrap();
    let eject = items[0]
        .actions
        .iter()
        .find(|a| a.operation == Operation::Eject)
        .unwrap();
    let regions = &items[1].regions;
    assert_eq!(regions.iter().map(|r| r.bytes).sum::<u64>(), items[1].size);
    assert!(regions.iter().any(|r| r.free));
    fs::write(d.join("mounted"), "").unwrap();
    assert!(f.client.perform(power, "").is_err());
    assert!(!d.join("powered-off").exists());
    fs::remove_file(d.join("mounted")).unwrap();
    let unlock = items.last().unwrap().actions.first().unwrap();
    fs::write(d.join("deny"), "").unwrap();
    assert!(f.client.perform(unlock, "survey").is_err());
    fs::remove_file(d.join("deny")).unwrap();
    assert!(f.client.perform(unlock, "wrong").is_err());
    f.client.perform(unlock, "survey").unwrap();
    assert!(f.client.perform(power, "").is_err());
    assert!(f.client.perform(unlock, "survey").is_err());
    fs::remove_file(d.join("unlocked")).unwrap();
    f.client.perform(eject, "").unwrap();
    assert!(f.client.perform(power, "").is_err());
    fs::remove_file(d.join("ejected")).unwrap();
    f.client.perform(power, "").unwrap();
    assert!(d.join("powered-off").exists());
}
