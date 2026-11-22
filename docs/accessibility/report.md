# Linux accessibility acceptance

All four applications use GPUI’s accessible application constructor at Zed `f4fe0be1`. GTK and the suite registered on the private accessibility bus. Files, Text Editor and System Monitor passed their live control tests. All 20 Rust tests passed and all 26 reviewed screenshots matched pixel for pixel. Three assertions keep the milestone open.

| Release check | Observed result |
| --- | --- |
| Disabled button | The field fixture publishes AccessKit’s disabled flag, removes Click and guards the command. The command log stayed empty. AT-SPI still reported the button as sensitive. In the pinned Unix mapping, a role without read-only support falls through to Enabled and Sensitive. [Adapter state mapping](https://docs.rs/accesskit_atspi_common/0.19.1/src/accesskit_atspi_common/node.rs.html) |
| Orca character echo | Orca 51.1 read “Find text entry Tokyo”, “Find match button” and “Find in terminal heading”. After closing Find it read the shell prompt. Its Where Am I command returned “Shell terminal $”. With character, key and alphabetic echo enabled, typing `z` produced an AT-SPI insert at offset 5 and a caret at 6. Orca classified the reason as unknown and emitted no utterance for that character. It did speak individual characters in the shell. [Timestamped speech-server requests](orca-terminal.jsonl) |
| Cached subtree | The fresh snapshot contained the window, a list box and twenty options. A redraw with the GPUI view cache reused the pixels and submitted only the window, with an empty child list. The private bus carried a children-changed signal and 21 state changes during the cached phase. The suite’s virtual lists use ordinary uncached GPUI elements. [Frame measurements](frame-cost.json) |

## Controls and text

Shared controls now publish roles and names independently of their identity. Files keys rows by their native paths. System Monitor uses PID and process start time. In the monitor fixture the inspector followed the same accessible process across a sort, scrolled the active option across a viewport boundary, checked its bounds and restored the selection by AT-SPI Click. A modal removes its covered controls from navigation. When the modal closes, the newly exposed content receives a fresh accessibility identity so Orca can discard its defunct references.

TextField publishes stable runs per logical line, subdividing long lines to fit AccessKit’s byte-sized word-start indices. It uses the shaped viewport for character geometry and includes offscreen text for document queries. The remote selection translates from run positions to model byte offsets. The live tests cover empty text, combining marks, non-BMP characters, CRLF, directional and collapsed selections, and replacing a remote selection via keyboard input. They check insert, delete, caret and selection events. A GPUI test exercises IME preedit, commit and cancellation. The password fixture exposes bullets for its text, runs and change events. Its real value stays in the field model.

The two-window fixture enables the adapter after map, disables and re-enables it, compares the republished tree, then closes a window with two outstanding activation requests. Exactly one command reached the log. Terminal also runs with accessibility enabled before map. Its test checks modal ancestry, current focus, scaled character geometry, window- and parent-relative extents, point lookup, the static heading’s Text interface and restored focus.

## Reproduction

Build the apps and examples once. Each command below creates disposable HOME and XDG directories, a private session bus, a separate AT-SPI launcher and registry, and headless Sway using Pixman. GPUI renders with Mesa lavapipe. The observer stays registered while Orca enumerates the GTK control and the GPUI process. Orca loads a recording speech server from its disposable profile. Logs, event order, observed trees and transcripts go under `target/wayland/accessibility-*`. The failing release checks return a nonzero exit status. [Wayland dependencies](../terminal-tests.md#software-rendered-wayland-tests)

```sh
cargo build --locked --workspace --all-targets
cargo test --locked --workspace
tests/wayland/run --fields
tests/wayland/run --at-spi --app terminal
tests/wayland/run --at-spi --app files
tests/wayland/run --at-spi --app text-editor
tests/wayland/run --at-spi --app system-monitor
```

The recorded output was rendered offline with eSpeak NG. A human listen-through is pending. The mundaneDE launcher and Settings owners will run their speech scenarios in their respective projects. The joe Linux image smoke test remains pending on packaging.

## Frame cost

The release fixture draws 20 visible rows from 100,000 on an Intel Core Ultra 7 265KF, at scale 1. It uses the same layout, model and operations at both revisions. Within each trial the driver randomizes the order of the May baseline, the selected pin with assistive technology inactive, and the same pin with the adapter active. Each operation gets 50 warm-up frames and 1,000 measured frames. The report uses thread CPU time for synchronous foreground GPUI frame construction. It records foreground allocations, process CPU less foreground CPU, per-trial median and p95, and paired upper one-sided 95% bootstrap limits across 20 trials. The edit limit compares enabled and inactive p95 on the selected pin. The inactive limit compares median frame time against May.

All four inactive upper bounds met the 1% target. Across 20 activations, first-snapshot latency was 2.16 ms at the median and 46.55 ms at p95. The upper bound for the additional p95 cost of a one-row edit was 0.022 ms against the 0.5 ms limit. The inactive trace recorded zero row-annotation allocations and zero submitted updates. The active trace sent 22 nodes for each edit or scroll. Cached redraws failed the tree-preservation check above.

The untimed trace build instruments disposable copies of GPUI and its Unix adapter. It counts producer nodes and the JSON size of each in-memory payload, samples the adapter event-loop thread clock, and captures actual AT-SPI object signals in D-Bus pcap format. The byte counts for those two boundaries are reported separately. The collector attributes signals by timestamp, so asynchronous delivery can cross phase boundaries. The first-snapshot clock runs from the adapter’s activation callback to submission of the first completed frame. Cost here measures the list fixture; large multiline documents also publish offscreen runs.

```sh
work=$(mktemp -d)
tests/cost/build "$work" --trace
COST_TRIALS=20 ACTIVATION_TRIALS=20 tests/wayland/run --cost "$work"
python3 -B tests/cost/report.py "$work"
```

## Typing echo under mundaneDE

The Terminal Find check passed under mundaneDE main at `2a0c8d0`. Its production panel launched Orca and authorized its keyboard monitor. A standard US evdev key from the private Sway host reached the nested winit seat. Find changed from “Tokyo” to “Tokyoz”, AT-SPI reported an insertion of “z” at offset 5 and the caret at 6, and Orca’s recording speech backend said **“z”**. The Sway-only variant above retains its original assertion. [Recorded speech](orca-mundanede.jsonl)

Build a scratch clone of mundaneDE with `cargo build --locked --bin mundane-de --example nested-input`. From this repository run `tests/wayland/mundanede-echo /path/to/scratch/clone` with the Wayland test dependencies, bubblewrap, xkbcli, mundaneDE panel dependencies and Orca on PATH. The wrapper gives the panel a private session and system-bus substitute, headless output, temporary home and PID namespace.
