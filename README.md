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

## Files

Files browses local directories, opens files in the preferred application, and copies, moves, renames or trashes a selected item. A name collision offers Cancel or Keep both. Visible rows use cached thumbnails, generate previews for PNG and JPEG files, or display a system MIME icon.

| Action | Shortcut |
| --- | --- |
| Select an item and open it | ↑ / ↓, then Enter |
| Go to the parent folder / enter a folder path | Alt+↑ / Ctrl+L |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V |
| Rename / move to Trash | F2 / Delete |
| Refresh / show or hide hidden items | F5 / Ctrl+H |
| Cycle focus / dismiss a dialog | Tab or Shift+Tab / Escape |

Both apps use the shared window header. With client-side decorations, F6 reaches the header and Tab reaches the available window controls. Drag the title to move the window or double-click it to maximize or restore. [Files test report](docs/files-tests.md)

## Building

Use a Rust toolchain with Cargo, a C/C++ toolchain, pkg-config, and the Linux development files for Wayland, xkbcommon and fontconfig. The release build was tested with Rust 1.95 on x86-64 Linux. Run Terminal in a Wayland session with Mesa, a Vulkan driver, installed fonts and xterm-256color terminfo.

```sh
cargo build --workspace --release --locked
./target/release/mundane-terminal
./target/release/mundane-files "$HOME"
```

Files uses the OpenURI desktop portal or GIO's default handler, GIO Trash, and GNU coreutils for recursive copies and cross-filesystem moves. Its desktop entry handles local directories. Distribution packaging and the joe image smoke test are pending. [Development plan](docs/plan.md)

## Licence

Original code and documentation use the [BSD 3-Clause licence](LICENSE). The shared text field adapts GPUI's text input example from Zed revision `deb48ceb47125223df73902d1c3b72f2f442222b`. That upstream code retains its [Apache 2.0 licence](LICENSES/Apache-2.0.txt). Fonts are resolved from the system.
