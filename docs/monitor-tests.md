# Testing System Monitor

Run `cargo test --workspace --locked` as a regular desktop user. The procfs test feeds successive samples with a process using 125% CPU, an exiting process and a reused PID. It checks user and system time, elapsed time, the kernel page-size conversion, unusual process names and the first interval for a new identity. CPU shows a dash until two samples are available for the same PID and start time. [Linux process accounting fields](https://man7.org/linux/man-pages/man5/proc_pid_stat.5.html)

A separate test spawns a private child, rejects a mismatched start time, sends SIGTERM to the verified child and checks its exit signal. The Linux service checks the start time through an opened process directory and signals through that same descriptor. The kernel enforces the current user's permissions. [Linux process-descriptor signals](https://man7.org/linux/man-pages/man2/pidfd_send_signal.2.html)

The GPUI headless test sorts by CPU, selects a process, cancels once and confirms once against a recording service. It advances the clock with a second confirmation open and reuses the PID. The selection clears, focus returns to Cancel and the replacement receives no request.

## Wayland acceptance

Build the installed binary and the test driver, then run the six compositor and scale combinations.

```sh
cargo build --locked --release -p mundane-system-monitor
cargo build --locked --release -p mundane-system-monitor --example monitor-fixture
export MUNDANE_SYSTEM_MONITOR_BIN="$PWD/target/release/mundane-system-monitor"
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app system-monitor --compositor "$compositor" --profile "$profile" || exit 1
    done
done
```

Each run opens the release binary on live procfs in a private session, resizes the window, operates the list by keyboard and closes it. The driver then opens the same view with deterministic counters and a recording signal service. The six references show the list sorted by CPU with `file-indexer` selected at 46.5% CPU and 128.0 MiB. The runner compares the pixels, cancels a termination, confirms the next one and checks the recorded PID and start time.

Sway and labwc use headless outputs with Pixman. GPUI uses Vulkan through Mesa lavapipe. The profiles cover 100%, 150% and 200% scaling, with high contrast and enlarged row text at 200%. The [joe image report](vm/182cd7c/README.md) includes the packaged GNOME Wayland check. [Test dependencies](terminal-tests.md#software-rendered-wayland-tests)

Resources samples aggregate CPU counters from `/proc/stat`, memory used as total minus available from `/proc/meminfo`, and receive/transmit byte deltas for non-loopback interfaces from `/proc/net/dev`. Network rates use elapsed sample time. Counter resets and unavailable readings leave a gap. Each graph exposes its current value, units and scale. The private Wayland fixture supplies deterministic counters and the GPUI tests cover name/PID filtering and selection across refreshes.
