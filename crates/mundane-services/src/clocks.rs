//! World clocks use the distribution's IANA database. Countdown deadlines use
//! Linux boot time, which includes suspend and is independent of wall-clock changes.
use anyhow::{Context, Result, bail, ensure};
use jiff::{
    Timestamp,
    tz::{self, TimeZone},
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::Value,
};

pub mod alarms;
pub use jiff::tz::TimeZone as LocalZone;

pub fn zones() -> Vec<String> {
    let mut zones: Vec<_> = tz::db()
        .available()
        .map(|n| n.to_string())
        .filter(|n| {
            n == "UTC"
                || n.contains('/')
                    && !["Etc/", "posix/", "right/", "SystemV/"]
                        .iter()
                        .any(|prefix| n.starts_with(prefix))
        })
        .collect();
    zones.sort();
    zones
}
pub fn wall_now() -> i64 {
    Timestamp::now().as_second()
}
pub fn boot_now() -> Duration {
    let t = rustix::time::clock_gettime(rustix::time::ClockId::Boottime);
    Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
}
pub struct City {
    pub zone: String,
    rules: TimeZone,
}
impl City {
    pub fn get(zone: &str) -> Result<Self> {
        Ok(Self {
            zone: zone.into(),
            rules: tz::db().get(zone)?,
        })
    }
    pub fn display(&self, unix: i64) -> (String, String) {
        let z = Timestamp::from_second(unix)
            .expect("valid wall timestamp")
            .to_zoned(self.rules.clone());
        (
            z.strftime("%H:%M").to_string(),
            z.strftime("%a, %d %b · %Z (UTC%:z)").to_string(),
        )
    }
}
pub fn city_name(zone: &str) -> String {
    zone.rsplit('/').next().unwrap_or(zone).replace('_', " ")
}
pub fn settings_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("mundane-clocks/cities"))
}
pub fn load(path: &Path) -> Result<Vec<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(s.lines().map(str::to_owned).collect()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(vec!["UTC".into()]),
        Err(e) => Err(e.into()),
    }
}
pub fn save_change(path: &Path, zone: &str, add: bool) -> Result<Vec<String>> {
    City::get(zone)?;
    let dir = path.parent().context("Missing settings directory")?;
    fs::create_dir_all(dir)?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("cities.lock"))?;
    lock.lock()?;
    let mut list = load(path)?;
    if add {
        if !list.iter().any(|z| z == zone) {
            list.push(zone.into())
        }
    } else {
        list.retain(|z| z != zone)
    }
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    for z in &list {
        writeln!(tmp, "{z}")?
    }
    tmp.persist(path)?;
    Ok(list)
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Countdown {
    #[default]
    Idle,
    Running(Duration),
    Finished,
}
impl Countdown {
    pub fn start(&mut self, text: &str, now: Duration) -> Result<()> {
        let parts: Vec<_> = text.trim().split(':').collect();
        ensure!(
            parts
                .iter()
                .all(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())),
            "Enter seconds, m:ss or h:mm:ss."
        );
        ensure!(parts.len() <= 3, "Enter seconds, m:ss or h:mm:ss.");
        let mut total = 0u64;
        for (i, v) in parts.iter().enumerate() {
            let n = v.parse::<u64>()?;
            ensure!(
                i == 0 || n < 60,
                "Minutes and seconds after a colon must be below 60."
            );
            total = total
                .checked_mul(60)
                .and_then(|t| t.checked_add(n))
                .context("Timer duration is too large.")?;
        }
        if total == 0 {
            bail!("Enter a duration greater than zero.")
        }
        *self = Self::Running(
            now.checked_add(Duration::from_secs(total))
                .context("Timer duration is too large.")?,
        );
        Ok(())
    }
    pub fn expire(&mut self, now: Duration) -> bool {
        if matches!(self,Self::Running(end) if now>=*end) {
            *self = Self::Finished;
            true
        } else {
            false
        }
    }
    pub fn remaining(self, now: Duration) -> String {
        let t = match self {
            Self::Running(end) => end.saturating_sub(now),
            _ => Duration::ZERO,
        };
        let n = t.as_secs() + u64::from(t.subsec_nanos() != 0);
        if n >= 3600 {
            format!("{}:{:02}:{:02}", n / 3600, n / 60 % 60, n % 60)
        } else {
            format!("{}:{:02}", n / 60, n % 60)
        }
    }
}
pub struct Notification {
    conn: Connection,
    owner: String,
    id: u32,
}
impl Notification {
    pub fn send() -> Result<Self> {
        Self::named("Timer finished", "The countdown has ended.")
    }
    pub fn named(summary: &str, body: &str) -> Result<Self> {
        let conn = Connection::session()?;
        let mut hints = HashMap::<&str, Value>::new();
        if summary == "Alarm" {
            hints.insert("sound-name", Value::from("alarm-clock-elapsed"));
            hints.insert("urgency", Value::from(2u8));
        }
        let reply = conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "Clocks",
                0u32,
                "preferences-system-time",
                summary,
                body,
                Vec::<String>::new(),
                hints,
                0i32,
            ),
        )?;
        let owner = reply
            .header()
            .sender()
            .context("Missing notification service owner")?
            .to_string();
        let id = reply.body().deserialize()?;
        Ok(Self { conn, owner, id })
    }
    pub fn close(self) -> Result<()> {
        Proxy::new(
            &self.conn,
            self.owner.as_str(),
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )?
        .call::<_, _, ()>("CloseNotification", &(self.id,))?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dst_jump_and_countdown() {
        let city = City::get("America/New_York").unwrap();
        let unix = "2025-03-09T06:59:59Z"
            .parse::<Timestamp>()
            .unwrap()
            .as_second();
        assert_eq!(city.display(unix).0, "01:59");
        assert_eq!(city.display(unix + 1).0, "03:00");
        let t = Duration::from_secs(100);
        let mut timer = Countdown::Idle;
        for bad in ["", "0", "1:60", "-2", "18446744073709551616"] {
            assert!(timer.start(bad, t).is_err())
        }
        timer.start("1:01", t).unwrap();
        assert_eq!(timer.remaining(t + Duration::from_millis(60200)), "0:01");
        assert!(timer.expire(t + Duration::from_secs(61)));
        assert!(!timer.expire(t + Duration::from_secs(100)));
    }
}
