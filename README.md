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

Set `MUNDANE_HIGH_CONTRAST=1` for the high-contrast palette. The suite now uses GPUI’s upstream AccessKit bridge. The apps expose their shared controls through AT-SPI. Disabled-button state, Orca character echo in Find and cached GPUI subtrees have open release checks. [Accessibility results and recording](docs/accessibility/report.md)

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

The suite uses a shared window header. With client-side decorations, F6 reaches the header and Tab reaches the available window controls. Drag the title to move the window or double-click it to maximize or restore. [Files test report](docs/files-tests.md)

## Text Editor

Open a local UTF-8 file or start with an untitled document. Find highlights a match and wraps at the end of the document. The title shows unsaved changes. Close or open another file to choose Save, Discard or Cancel. If a file changes on disk, Save As lets you keep your edits in another file. [Text Editor tests](docs/editor-tests.md)

| Action | Shortcut |
| --- | --- |
| Open / save / save as | Ctrl+O / Ctrl+S / Ctrl+Shift+S |
| Find / next match / leave Find | Ctrl+F / Enter in Find or F3 / Escape |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z |
| Start or end of line / document | Home or End / Ctrl+Home or Ctrl+End |
| Increase / decrease / reset document text size | Ctrl++ / Ctrl+- / Ctrl+0 |
| Close document | Ctrl+W |

## System Monitor

System Monitor samples visible processes every two seconds. CPU is measured across successive samples, with 100% representing one logical CPU. Memory shows approximate resident size. Select a process and choose **End process…** to review its name and PID before sending SIGTERM. [System Monitor tests](docs/monitor-tests.md)

| Action | Shortcut |
| --- | --- |
| Sort by process name, PID, CPU or memory | N, P, C or M |
| Reverse the current sort | Activate the same column again |
| Select a process | ↑ / ↓, Home or End |
| Request termination / cancel | Delete / Escape |
| Increase / decrease / reset row text size | Ctrl++ / Ctrl+- / Ctrl+0 |

## Image Viewer

Open a local PNG or JPEG from Files, the command line, or the **Open** button. The viewer applies embedded orientation, fits the whole image to the window, and shows its upright dimensions. Zoom is relative to that fit. A failed Open leaves the current image available. [Image Viewer tests](docs/viewer-tests.md)

| Action | Shortcut |
| --- | --- |
| Open / close | Ctrl+O / Ctrl+W |
| Zoom in / out | + / − (also Ctrl+= / Ctrl+-) |
| Pan a zoomed image | Arrow keys or scroll |
| Fit to window | F or Ctrl+0 |
| Reach Open / return to the image | F6 / Escape |

## Calculator

Enter an arithmetic expression or use the keypad. Calculator handles parentheses and decimal arithmetic exactly. It displays repeating decimals as reduced fractions. Choose **Use result** to continue a calculation or select an earlier history row to reuse its answer. The window keeps the latest 50 successful calculations. **Clear history** leaves the current expression and result visible. [Calculator tests](docs/calculator-tests.md)

| Action | Shortcut |
| --- | --- |
| Evaluate the expression | Enter |
| Use the displayed result | Ctrl+R |
| Reach the newest history entry | Alt+H |
| Clear the expression / close Calculator | Ctrl+L / Ctrl+W |
| Reach the header / return to the expression | F6 / Escape |

## Packages

Use **Installed** to browse local packages, or search for a name or description. Open a result to inspect dependencies and review installation or removal. **Updates** shows the running deployment, the next boot, and upgrades prepared for shutdown. Each change requires confirmation of a specific dave plan. [Packages tests and polkit contract](docs/packages-tests.md)

| Action | Shortcut |
| --- | --- |
| Search packages | Ctrl+F, type, then Enter |
| Open Updates / refresh the current view | Ctrl+U / F5 |
| Focus the header | F6 |
| Cancel review or return from details | Escape |
| Browse a longer plan | Page Up / Page Down |
| Increase, decrease or reset text size | Ctrl++ / Ctrl+- / Ctrl+0 |

## Document Viewer

Open a local PDF from Files, the command line, or the **Open** button. Jump to a page, search across the document, and zoom in to read a match. The page exposes its embedded text to screen readers. Document Viewer loads the system PDFium shared library at runtime. [Engine comparison, installation and tests](docs/document-tests.md)

| Action | Shortcut |
| --- | --- |
| Open / close | Ctrl+O / Ctrl+W |
| Previous / next page | Page Up / Page Down |
| Enter a page number | Ctrl+L, then Enter |
| Find text | Ctrl+F, type a phrase, then Enter |
| Next / previous match | F3 / Shift+F3 |
| Zoom in / out / return to 100% | Ctrl+= / Ctrl+- / Ctrl+0 |
| Scroll / leave Find | Arrow keys on the page / Escape |

## Video

Video plays local WebM files with VP8 or VP9 video and optional Opus or Vorbis audio. Sound goes directly to PipeWire. Click or drag the seek slider, use the five-second step buttons, or operate the player by keyboard. At the end of a clip, **Replay** starts it again. [GStreamer packaging list and Video tests](docs/video-tests.md)

| Action | Shortcut |
| --- | --- |
| Open / close | Ctrl+O / Ctrl+W |
| Play or pause with the picture focused | Space |
| Step back / forward five seconds | Left / Right |
| Seek to start / end | Home / End |
| Reach Open / return to the picture | F6 / Escape |

## Building

Use a Rust toolchain with Cargo, a C/C++ toolchain, pkg-config, and the Linux development files for Wayland, xkbcommon and fontconfig. The release build was tested with Rust 1.100 on x86-64 Linux. Run Terminal in a Wayland session with Mesa, a Vulkan driver, installed fonts and xterm-256color terminfo.

```sh
cargo build --workspace --release --locked
./target/release/mundane-terminal
./target/release/mundane-files "$HOME"
./target/release/mundane-text-editor
./target/release/mundane-system-monitor
./target/release/mundane-image-viewer "tests/fixtures/North Point.jpg"
./target/release/mundane-calculator
./target/release/mundane-packages
./target/release/mundane-document-viewer "tests/fixtures/Coastal survey.pdf"
./target/release/mundane-video "tests/fixtures/Coastal passage.webm"
```

Files uses the OpenURI desktop portal or GIO's default handler, GIO Trash, and GNU coreutils for recursive copies and cross-filesystem moves. Its desktop entry handles local directories. Text Editor accepts a local path and declares `text/plain`. Its Open and Save As dialogs use the FileChooser desktop portal. Distribution packaging and the joe image smoke test are pending. [Development plan](docs/plan.md)

## Licence

Original code and documentation use the [BSD 3-Clause licence](LICENSE). The shared text field adapts GPUI's text input example from Zed revision `deb48ceb47125223df73902d1c3b72f2f442222b`. That upstream code retains its [Apache 2.0 licence](LICENSES/Apache-2.0.txt). Fonts are resolved from the system.
