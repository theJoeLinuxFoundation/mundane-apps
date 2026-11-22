//! Host totals from procfs. Network deltas compare matching non-loopback interfaces.
use std::{collections::BTreeMap, fs, time::Instant};

pub struct ResourceSample {
    pub at: Instant,
    pub cpu: Option<(u64, u64)>,    // non-idle ticks and total ticks
    pub memory: Option<(u64, u64)>, // used and total bytes
    pub interfaces: Option<BTreeMap<String, (u64, u64)>>, // receive and transmit counters
}
fn cpu(stat: &str) -> Option<(u64, u64)> {
    let mut cols = stat
        .lines()
        .find(|s| s.starts_with("cpu "))?
        .split_whitespace()
        .skip(1);
    let vals: Vec<u64> = cols
        .by_ref()
        .take(8)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let idle = vals
        .get(3)?
        .checked_add(vals.get(4).copied().unwrap_or(0))?;
    let total = vals.iter().try_fold(0u64, |a, b| a.checked_add(*b))?;
    Some((total.checked_sub(idle)?, total))
}
fn memory(info: &str) -> Option<(u64, u64)> {
    let field = |name: &str| {
        info.lines()
            .find_map(|line| line.strip_prefix(name))?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()?
            .checked_mul(1024)
    };
    let total = field("MemTotal:")?;
    (total > 0).then_some((total.checked_sub(field("MemAvailable:")?)?, total))
}
fn network(dev: &str) -> Option<BTreeMap<String, (u64, u64)>> {
    let mut interfaces = BTreeMap::new();
    for line in dev.lines().skip(2) {
        let (name, counters) = line.rsplit_once(':')?;
        if name.trim() == "lo" {
            continue;
        }
        let mut cols = counters.split_whitespace();
        let rx = cols.next()?.parse().ok()?;
        let tx = cols.nth(7)?.parse().ok()?;
        interfaces.insert(name.trim().to_owned(), (rx, tx));
    }
    Some(interfaces)
}
impl ResourceSample {
    pub(super) fn read() -> Self {
        Self {
            at: Instant::now(),
            cpu: fs::read_to_string("/proc/stat").ok().and_then(|s| cpu(&s)),
            memory: fs::read_to_string("/proc/meminfo")
                .ok()
                .and_then(|s| memory(&s)),
            interfaces: fs::read_to_string("/proc/net/dev")
                .ok()
                .and_then(|s| network(&s)),
        }
    }
    pub fn usage(&self, prev: Option<&Self>) -> Usage {
        let cpu = (|| {
            let (busy, total) = self.cpu?;
            let (a, b) = prev?.cpu?;
            let dt = total.checked_sub(b)?;
            (dt > 0).then_some(100. * busy.checked_sub(a)?.min(dt) as f64 / dt as f64)
        })();
        let net = (|| {
            let prev = prev?;
            let dt = self.at.checked_duration_since(prev.at)?.as_secs_f64();
            if dt == 0. {
                return None;
            }
            let old = prev.interfaces.as_ref()?;
            let mut rx = 0.;
            let mut tx = 0.;
            for (name, (r, t)) in self.interfaces.as_ref()? {
                if let Some((a, b)) = old.get(name) {
                    rx += r.checked_sub(*a)? as f64;
                    tx += t.checked_sub(*b)? as f64;
                }
            }
            Some((rx / dt, tx / dt))
        })();
        Usage {
            cpu,
            memory: self.memory,
            network: net,
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct Usage {
    pub cpu: Option<f64>,
    pub memory: Option<(u64, u64)>,
    pub network: Option<(f64, f64)>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resources_exclude_guest_and_loopback_and_rebase_resets() {
        assert_eq!(cpu("cpu  20 0 10 60 5 1 2 2 200 4"), Some((35, 100)));
        assert_eq!(
            memory("MemTotal: 10 kB\nMemAvailable: 6 kB"),
            Some((4096, 10240))
        );
        let net = network(
            "header\ncolumns\n lo: 200 0 0 0 0 0 0 0 100 0\n eth0: 400 0 0 0 0 0 0 0 800 0",
        )
        .unwrap();
        assert_eq!(net.len(), 1);
        assert_eq!(net["eth0"], (400, 800));
        let a = ResourceSample {
            at: Instant::now(),
            cpu: Some((5, 10)),
            memory: None,
            interfaces: Some(net.clone()),
        };
        let mut b = ResourceSample {
            at: a.at + std::time::Duration::from_secs(2),
            cpu: Some((7, 20)),
            memory: None,
            interfaces: Some(net),
        };
        b.interfaces
            .as_mut()
            .unwrap()
            .insert("eth0".into(), (500, 1000));
        assert_eq!(b.usage(Some(&a)).cpu, Some(20.));
        assert_eq!(b.usage(Some(&a)).network, Some((50., 100.)));
        b.interfaces.as_mut().unwrap().insert("eth0".into(), (0, 0));
        assert!(b.usage(Some(&a)).network.is_none());
    }
}
