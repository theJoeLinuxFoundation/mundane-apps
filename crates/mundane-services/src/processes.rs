//! Linux process accounting and confirmed SIGTERM delivery.
mod resources;
pub use resources::{ResourceSample, Usage};
use rustix::{
    fs::{Mode, OFlags, openat},
    process::{Signal, pidfd_send_signal},
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Read},
    path::Path,
    time::Instant,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ProcessId {
    pub pid: u32,
    pub start_ticks: u64,
}

pub struct Counters {
    pub id: ProcessId,
    pub name: String,
    pub cpu_ticks: u64,
    pub resident_bytes: u64,
}

pub struct Sample {
    pub at: Instant,
    pub ticks_per_second: u64,
    pub processes: Vec<Counters>,
}

pub trait ProcessService: Send + Sync {
    fn resources(&self) -> Option<ResourceSample> {
        None
    }
    fn sample(&self) -> io::Result<Sample>;
    fn terminate(&self, id: ProcessId) -> io::Result<()>;
}

pub struct Procfs;

fn parse_stat(bytes: &[u8], page_size: u64) -> Option<Counters> {
    let stat = String::from_utf8_lossy(bytes);
    let (pid, rest) = stat.split_once(" (")?;
    // comm may itself contain spaces, parentheses and newlines.
    let (comm, rest) = rest.rsplit_once(')')?;
    let mut fields = rest.split_ascii_whitespace();
    let user: u64 = fields.nth(11)?.parse().ok()?; // field 14, after pid and comm
    let system: u64 = fields.next()?.parse().ok()?;
    let start_ticks = fields.nth(6)?.parse().ok()?; // field 22
    let pages: i64 = fields.nth(1)?.parse().ok()?; // field 24
    Some(Counters {
        id: ProcessId {
            pid: pid.parse().ok()?,
            start_ticks,
        },
        name: comm
            .chars()
            .map(|c| if c.is_control() { '�' } else { c })
            .collect(),
        cpu_ticks: user.checked_add(system)?,
        resident_bytes: (pages.max(0) as u64).checked_mul(page_size)?,
    })
}

impl ProcessService for Procfs {
    fn resources(&self) -> Option<ResourceSample> {
        Some(ResourceSample::read())
    }
    fn sample(&self) -> io::Result<Sample> {
        let at = Instant::now();
        let page_size = rustix::param::page_size() as u64;
        let mut processes = Vec::new();
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            // Tasks can exit or be hidden by procfs permissions while scanning.
            if let Ok(bytes) = fs::read(entry.path().join("stat"))
                && let Some(process) = parse_stat(&bytes, page_size)
                && process.id.pid == pid
            {
                processes.push(process);
            }
        }
        Ok(Sample {
            at,
            ticks_per_second: rustix::param::clock_ticks_per_second(),
            processes,
        })
    }

    fn terminate(&self, id: ProcessId) -> io::Result<()> {
        // Linux accepts an open /proc/PID directory as a process FD. Read stat
        // relative to that same handle, then signal the verified task through it.
        let dir = File::open(Path::new("/proc").join(id.pid.to_string()))?;
        let fd = openat(
            &dir,
            "stat",
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let mut bytes = Vec::new();
        File::from(fd).read_to_end(&mut bytes)?;
        if !parse_stat(&bytes, rustix::param::page_size() as u64).is_some_and(|p| p.id == id) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "The selected process has exited.",
            ));
        }
        pidfd_send_signal(&dir, Signal::TERM)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Process {
    pub id: ProcessId,
    pub name: String,
    pub cpu_percent: Option<f64>,
    pub resident_bytes: u64,
}

#[derive(Default)]
pub struct History {
    previous: Option<(Instant, HashMap<ProcessId, u64>)>,
}

impl History {
    pub fn update(&mut self, sample: Sample) -> Vec<Process> {
        let mut counters = HashMap::with_capacity(sample.processes.len());
        let mut rows = Vec::with_capacity(sample.processes.len());
        for p in sample.processes {
            let cpu_percent = self.previous.as_ref().and_then(|(at, ticks)| {
                let elapsed = sample.at.checked_duration_since(*at)?.as_secs_f64();
                let delta = p.cpu_ticks.checked_sub(*ticks.get(&p.id)?)?;
                (elapsed > 0. && sample.ticks_per_second != 0)
                    .then(|| 100. * delta as f64 / sample.ticks_per_second as f64 / elapsed)
            });
            counters.insert(p.id, p.cpu_ticks);
            rows.push(Process {
                id: p.id,
                name: p.name,
                cpu_percent,
                resident_bytes: p.resident_bytes,
            });
        }
        self.previous = Some((sample.at, counters));
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::process::ExitStatusExt, process::Command, time::Duration};

    fn stat(pid: u32, start: u64, user: u64, system: u64) -> Counters {
        let raw = format!(
            "{pid} (a ) task\n) R 1 2 3 4 5 6 7 8 9 10 {user} {system} 999 999 0 0 1 0 {start} 90000 17 0"
        );
        parse_stat(raw.as_bytes(), 8192).unwrap()
    }

    #[test]
    fn samples_account_for_elapsed_time_exit_and_pid_reuse() {
        let mut history = History::default();
        let at = Instant::now();
        let mut sample = |seconds, processes| {
            history.update(Sample {
                at: at + Duration::from_secs(seconds),
                ticks_per_second: 100,
                processes,
            })
        };
        let first = sample(
            0,
            vec![stat(1, 10, 20, 5), stat(2, 20, 500, 100), stat(3, 30, 5, 6)],
        );
        assert!(first.iter().all(|p| p.cpu_percent.is_none()));
        let second = sample(2, vec![stat(1, 10, 220, 55), stat(2, 40, 600, 200)]);
        assert_eq!(second.len(), 2);
        assert_eq!(second[0].cpu_percent, Some(125.0));
        assert_eq!(second[0].name, "a ) task�");
        assert_eq!(second[0].resident_bytes, 17 * 8192);
        assert_eq!(second[1].cpu_percent, None);
        let third = sample(4, vec![stat(1, 10, 220, 55), stat(2, 40, 620, 200)]);
        assert_eq!(third[0].cpu_percent, Some(0.));
        assert_eq!(third[1].cpu_percent, Some(10.));
        assert!(parse_stat(b"1 (incomplete) S 0", 4096).is_none());
    }

    #[test]
    fn verifies_identity_and_signals_a_private_child() {
        let mut child = Command::new("sleep").arg("10").spawn().unwrap();
        let sample = Procfs.sample().unwrap();
        let id = sample
            .processes
            .iter()
            .find(|p| p.id.pid == child.id())
            .unwrap()
            .id;
        assert!(
            Procfs
                .terminate(ProcessId {
                    start_ticks: id.start_ticks + 1,
                    ..id
                })
                .is_err()
        );
        assert!(child.try_wait().unwrap().is_none());
        Procfs.terminate(id).unwrap();
        assert_eq!(child.wait().unwrap().signal(), Some(15));
        assert!(Procfs.terminate(id).is_err());
    }
}
