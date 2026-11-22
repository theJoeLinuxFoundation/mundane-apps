//! The actual monitor view with repeatable samples and a recording signal service.
use mundane_services::processes::{Counters, ProcessId, ProcessService, Sample};
use std::{
    fs, io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Fixture {
    epoch: Instant,
    count: Mutex<u64>,
    output: PathBuf,
}

impl ProcessService for Fixture {
    fn sample(&self) -> io::Result<Sample> {
        let mut count = self.count.lock().unwrap();
        let step = *count;
        *count += 1;
        let mut processes = Vec::new();
        for (pid, name, ticks, mib) in [
            (2841, "video-encoder", 285, 416),
            (932, "mundane-desktop", 48, 178),
            (1736, "text-editor", 11, 64),
            (1978, "file-indexer", 93, 128),
            (1012, "pipewire", 6, 23),
            (1248, "terminal", 3, 47),
            (2114, "image-viewer", 1, 192),
            (810, "session-bus", 0, 4),
        ] {
            processes.push(Counters {
                id: ProcessId {
                    pid,
                    start_ticks: pid as u64 * 10,
                },
                name: name.into(),
                cpu_ticks: 300 + ticks * step,
                resident_bytes: mib << 20,
            });
        }
        // The driver waits for a completed sampling interval before sorting by CPU.
        if step == 1 {
            fs::write(self.output.join("sampled"), b"ready")?;
        }
        Ok(Sample {
            at: self.epoch + Duration::from_secs(2 * step),
            ticks_per_second: 100,
            processes,
        })
    }
    fn terminate(&self, id: ProcessId) -> io::Result<()> {
        use std::io::Write;
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.output.join("signals"))?;
        writeln!(log, "{} {}", id.pid, id.start_ticks)
    }
}

fn main() {
    let output = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").expect("private test runtime"));
    mundane_system_monitor::run(Arc::new(Fixture {
        epoch: Instant::now(),
        count: Mutex::new(0),
        output,
    }));
}
