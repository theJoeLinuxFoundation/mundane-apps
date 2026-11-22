# mundane-apps

Desktop applications for mundaneDE on the x86 edition of joe Linux. They use Rust and GPUI's Linux Wayland platform, with GPU rendering through wgpu.

## Terminal

Terminal runs the user's shell in a PTY. It supports scrollback, text selection, clipboard, search, and resizing. Drag to select text, double-click to select a word, or use Find to select a match from the keyboard.

| Action | Shortcut |
| --- | --- |
| Find text in the screen or scrollback | Ctrl+Shift+F, then Enter |
| Copy selection / paste | Ctrl+Shift+C / Ctrl+Shift+V |
| Scroll one page | Shift+Page Up / Shift+Page Down |
| Increase / decrease / reset terminal text size | Ctrl++ / Ctrl+- / Ctrl+0 |
| Focus the header or return to the shell | F6 |
| Dismiss Find | Escape |

Set `MUNDANE_HIGH_CONTRAST=1` for the high-contrast palette. Keyboard input and focus work in Terminal, but the pinned GPUI Linux backend does not expose its controls to AT-SPI. [Accessibility probe and test instructions](docs/terminal-tests.md)

## Building

Use a Rust toolchain with Cargo, a C/C++ toolchain, pkg-config, and the Linux development files for Wayland, xkbcommon and fontconfig. The release build was tested with Rust 1.95 on x86-64 Linux. Run Terminal in a Wayland session with Mesa, a Vulkan driver, installed fonts and xterm-256color terminfo.

```sh
cargo build --workspace --release --locked
./target/release/mundane-terminal
```

The workspace includes the shared UI controls and terminal session service. Files is the next planned application. Distribution packaging and the joe image smoke test are pending. [Development plan](docs/plan.md)

## Licence

Original code and documentation use the [BSD 3-Clause licence](LICENSE). The shared text field adapts GPUI's text input example from Zed revision `deb48ceb47125223df73902d1c3b72f2f442222b`. That upstream code retains its [Apache 2.0 licence](LICENSES/Apache-2.0.txt). Fonts are resolved from the system.
