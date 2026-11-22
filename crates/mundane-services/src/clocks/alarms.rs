//! Wall-clock alarms shared by the window and the session watcher.
use super::*;
use serde::{Deserialize, Serialize};

pub const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

#[derive(Clone, Serialize, Deserialize)]
pub struct Alarm {
    pub id: u64,
    pub name: String,
    pub hour: i8,
    pub minute: i8,
    pub days: u8,
    pub next: Option<i64>,
    pub snoozed: Option<i64>,
    pub ringing: bool,
}

impl Alarm {
    pub fn time(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
    pub fn repeat(&self) -> String {
        match self.days {
            0 => "Once".into(),
            127 => "Every day".into(),
            days => DAYS
                .iter()
                .enumerate()
                .filter(|(i, _)| days & (1 << i) != 0)
                .map(|(_, day)| *day)
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
    fn occurrence(&self, now: i64, zone: &TimeZone) -> Result<i64> {
        let mut date = Timestamp::from_second(now)?.to_zoned(zone.clone()).date();
        for _ in 0..8 {
            let time = date
                .at(self.hour, self.minute, 0, 0)
                .to_zoned(zone.clone())?
                .timestamp()
                .as_second();
            let weekday = date.weekday().to_monday_one_offset() - 1;
            if time > now && (self.days == 0 || self.days & (1 << weekday) != 0) {
                return Ok(time);
            }
            date = date.tomorrow()?;
        }
        bail!("No next alarm time")
    }
}
pub fn read(path: &Path) -> Result<Vec<Alarm>> {
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}
fn edit<T>(path: &Path, change: impl FnOnce(&mut Vec<Alarm>) -> Result<(T, bool)>) -> Result<T> {
    let dir = path.parent().context("No alarm settings directory")?;
    fs::create_dir_all(dir)?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("alarms.lock"))?;
    lock.lock()?;
    let mut alarms = read(path)?;
    let (result, changed) = change(&mut alarms)?;
    if changed {
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer(&mut tmp, &alarms)?;
        tmp.flush()?;
        tmp.persist(path)?;
    }
    Ok(result)
}
pub fn add(path: &Path, name: &str, time: &str, days: u8, now: i64, zone: &TimeZone) -> Result<()> {
    let name = name.trim();
    ensure!(
        !name.is_empty() && name.len() <= 160,
        "Enter an alarm name (up to 160 bytes)."
    );
    let (h, m) = time
        .trim()
        .split_once(':')
        .context("Enter a time as HH:MM.")?;
    let (hour, minute) = (h.parse::<i8>()?, m.parse::<i8>()?);
    ensure!(
        (0..24).contains(&hour) && (0..60).contains(&minute) && days <= 127,
        "Enter a time from 00:00 to 23:59."
    );
    edit(path, |list| {
        let id = list
            .iter()
            .map(|a| a.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .context("Too many alarms")?;
        let mut alarm = Alarm {
            id,
            name: name.to_owned(),
            hour,
            minute,
            days,
            next: None,
            snoozed: None,
            ringing: false,
        };
        alarm.next = Some(alarm.occurrence(now, zone)?);
        list.push(alarm);
        Ok(((), true))
    })
}
#[derive(Clone, Copy)]
pub enum Action {
    Remove,
    Snooze,
    Dismiss,
}
pub fn change(path: &Path, id: u64, action: Action, now: i64) -> Result<()> {
    edit(path, |list| {
        let i = list
            .iter()
            .position(|a| a.id == id)
            .context("This alarm was removed.")?;
        match action {
            Action::Remove => {
                list.remove(i);
            }
            Action::Snooze | Action::Dismiss => {
                ensure!(list[i].ringing, "This alarm is no longer ringing.");
                list[i].ringing = false;
                list[i].snoozed = matches!(action, Action::Snooze).then_some(now + 9 * 60);
            }
        }
        Ok(((), true))
    })
}
pub fn due(path: &Path, now: i64, zone: &TimeZone) -> Result<Vec<String>> {
    edit(path, |list| {
        let mut names = Vec::new();
        for alarm in list {
            let scheduled = alarm.next.is_some_and(|t| t <= now);
            if scheduled || alarm.snoozed.is_some_and(|t| t <= now) {
                if scheduled {
                    alarm.next = if alarm.days == 0 {
                        None
                    } else {
                        Some(alarm.occurrence(now, zone)?)
                    };
                }
                alarm.snoozed = None;
                alarm.ringing = true;
                names.push(alarm.name.clone());
            }
        }
        let changed = !names.is_empty();
        Ok((names, changed))
    })
}

/// One watcher per user, including when multiple windows add alarms concurrently.
pub fn watch(path: &Path) -> Result<()> {
    if read(path)?.is_empty() {
        return Ok(());
    }
    let dir = path.parent().context("No alarm settings directory")?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("watcher.lock"))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    loop {
        for name in due(path, wall_now(), &TimeZone::system())? {
            if let Err(e) = Notification::named("Alarm", &name) {
                eprintln!("Alarm notification: {e}");
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weekend_dst_snooze_and_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alarms.json");
        let zone = TimeZone::get("America/New_York").unwrap();
        let before = "2025-03-08T13:00:00Z"
            .parse::<Timestamp>()
            .unwrap()
            .as_second();
        add(&path, "Sunday bell", "08:00", 1 << 6, before, &zone).unwrap();
        let start = read(&path).unwrap()[0].next.unwrap();
        assert_eq!(start - before, 23 * 3600);
        assert_eq!(due(&path, start, &zone).unwrap(), ["Sunday bell"]);
        assert!(due(&path, start, &zone).unwrap().is_empty());
        change(&path, 1, Action::Snooze, start).unwrap();
        assert_eq!(due(&path, start + 540, &zone).unwrap(), ["Sunday bell"]);
        change(&path, 1, Action::Dismiss, start + 540).unwrap();
        assert_eq!(read(&path).unwrap()[0].next, Some(start + 7 * 86400));
    }
}
