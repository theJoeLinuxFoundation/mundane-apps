# Testing Terminal

## Shell and GPUI tests

Run `cargo test --workspace --locked`. A parser replay splits UTF-8 and escape sequences across reads and checks wide-cell occupancy, colour, cursor movement and scrollback. A Linux PTY test resizes a live shell, checks the dimensions reported by `stty`, waits for its exit event and checks that its PID has been reaped.

The GPUI headless test types a command into a real PTY, waits for the output row, drags across it and checks the test clipboard. It also exercises keyboard search, tab traversal, focus restoration and the selected text.

## Software-rendered Wayland tests

Build the binary once. The commands below use a new Sway compositor, Wayland socket, seat, clipboard, D-Bus session and home directory for each run. Sway uses wlroots' headless backend and Pixman renderer. The GPUI client uses Mesa lavapipe for Vulkan on the CPU.

```sh
cargo build --locked -p mundane-terminal
tests/wayland/run --at-spi
tests/wayland/run --profile fractional
tests/wayland/run --profile contrast
```

Use `MUNDANE_TERMINAL_BIN` to select an alternate build of the executable. The runner requires Sway and swaymsg, wtype, grim, wl-clipboard, Mesa lavapipe, ImageMagick 7, Python 3, Bash, ncurses utilities and D-Bus. Install DejaVu Sans, DejaVu Sans Mono and Noto Sans CJK. The font fixture selects specific files from the system. `TEST_FONT_DIR` can point to an unpacked font package. `VK_DRIVER_FILES` can point to an unpacked lavapipe ICD manifest. The AT-SPI probe additionally needs AT-SPI2, GTK 3, their Python GObject introspection bindings and Orca.

Each profile starts at 800 × 500 logical pixels and resizes to 640 × 360. The test types commands through the compositor, checks the resized PTY dimensions, finds text in the output, verifies the selection on the private clipboard and exits the shell. It compares compositor captures pixel for pixel with the checked-in references at 100%, 150% and 200% scale. The 200% profile enables high contrast, increases the terminal text size and captures the focused search field as well. Review a changed capture in `target/wayland/` alongside its counterpart in `tests/wayland/` before replacing a reference.

The reference environment used Sway 1.12, Mesa 26.2.5, grim 1.5.0 and wtype 0.4. Differences in fonts or rasterizer versions can change pixels.

## AT-SPI probe result

With Terminal's Find dialog open, the test focuses the search field, moves focus to the Find match button and returns to the field. It inspects AT-SPI's desktop tree by application PID and asks Orca for its running applications. A separate GTK window exposes a named button as a positive control on the same private bus.

With AT-SPI2 2.62.1 and Orca 51.1, the inspector found the GTK application, its frame and the button named “Probe button”. Orca listed the GTK application. Terminal had no application root in either inspection. Consequently neither tool could read the field text, observe focus changes, discover the dialog role or activate the button through AT-SPI. Keyboard selection and activation are covered separately by the GPUI and Wayland tests.

Those observations were made at the milestone 1 pin, Zed `deb48ceb`. For current results at `f4fe0be1`, including the recorded Terminal Find transcript, see the shared accessibility report. [Current acceptance results](accessibility/report.md)

The joe Linux VM smoke test is pending until the distribution packages mundaneDE and Terminal together.

## Window controls

Terminal requests client decorations. The shared header reads the negotiated decoration mode and the toplevel's window-management capabilities. F6 enters the header, Tab and Shift+Tab traverse its available controls, and Enter or Space activates the focused button. Double-click the title to maximize or restore. Drag the title to move the window.

Tiled Sway negotiates server decorations in the reference configuration. An additional headless labwc run negotiates client decorations and advertises minimize and maximize. Both compositors use Pixman and the same lavapipe client. Install labwc, wlr-randr and wlrctl for this run. Build the virtual pointer test helper once, then run each profile.

```sh
cargo build -p mundane-terminal --example header-pointer --locked
tests/wayland/run --compositor labwc
tests/wayland/run --compositor labwc --profile fractional
tests/wayland/run --compositor labwc --profile contrast
```

The labwc references show the three controls. The test checks keyboard minimize, maximize, restore and close, and sends a double-click and drag through a virtual pointer on the private socket.
