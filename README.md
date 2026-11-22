# mundane-apps

Desktop applications for mundaneDE on the x86 edition of joe Linux. They use Rust and GPUI's Linux Wayland platform, with GPU rendering through wgpu.

All 18 apps follow the desktop’s light or dark preference, accent, contrast and reduced motion live through the XDG Settings portal. [Appearance behaviour and screenshots](docs/appearance.md)

The shell, Settings and the suite share the colour roles, typography, keyboard behaviour and accessible components in mundane-ui. [Design language and component reference sheets](docs/design-language.md)

The suite’s visual review includes keyboard focus, empty states, errors and the Files conflict dialog. [Before-and-after screenshots](docs/polish/README.md)

## Terminal

Terminal runs the user's shell in a PTY. It supports scrollback, text selection, clipboard, search, and resizing. Drag to select text, double-click to select a word, or use Find to select a match from the keyboard. Profiles store the font, palette and scrollback limit. Closing a tab or window with running jobs asks for confirmation.

| Action | Shortcut |
| --- | --- |
| Open / close a shell tab | Ctrl+Shift+T / Ctrl+Shift+W |
| Previous / next tab | Ctrl+Page Up / Ctrl+Page Down |
| Edit terminal profiles | Ctrl+Shift+P |
| Open a web link | Ctrl+click or Ctrl+Enter on a selected URL |
| Find text in the screen or scrollback | Ctrl+Shift+F, then Enter |
| Copy selection / paste | Ctrl+Shift+C / Ctrl+Shift+V |
| Scroll one page | Shift+Page Up / Shift+Page Down |
| Increase / decrease / reset terminal text size | Ctrl++ / Ctrl+- / Ctrl+0 |
| Focus the header or return to the shell | F6 |
| Dismiss Find | Escape |

The suite now uses GPUI’s upstream AccessKit bridge. The apps expose their shared controls through AT-SPI. Orca speaks typed characters in Terminal Find under nested mundaneDE. Headless Sway lacks the keyboard monitor needed for this check. The disabled-button state and cached-subtree checks remain open. [Accessibility results and recording](docs/accessibility/report.md)

## Files

Use the places sidebar for local folders, bookmarks and mounted media. Ctrl+D toggles the current bookmark, Ctrl+F searches filenames below the current folder, and Escape returns to browsing. [GNOME parity and screenshots](docs/parity.md)

Files browses local directories, opens files in the preferred application, and can copy, move or trash multiple items together. Shift extends a selection, Ctrl+Space toggles one item and Ctrl+A selects all. Drop the selection on a folder to move it, or hold Ctrl to copy it. When names collide, Files shows the size and modification date of the incoming and existing items. Choose Cancel, Keep both, or Replace. Cancel has initial focus. Visible rows use cached thumbnails, generate previews for PNG and JPEG files, or display a system MIME icon.

| Action | Shortcut |
| --- | --- |
| New / close folder tab | Ctrl+T / Ctrl+W |
| Previous / next folder tab | Ctrl+Page Up / Ctrl+Page Down |
| Select an item and open it | ↑ / ↓, then Enter |
| Go to the parent folder / enter a folder path | Alt+↑ / Ctrl+L |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V |
| Rename / move to Trash | F2 / Delete |
| Refresh / show or hide hidden items | F5 / Ctrl+H |
| Cycle focus / dismiss a dialog | Tab or Shift+Tab / Escape |

The suite uses a shared window header. With client-side decorations, F6 reaches the header and Tab reaches the available window controls. Drag the title to move the window or double-click it to maximize or restore. [Files test report](docs/files-tests.md)

## Text Editor

Open a local UTF-8 file or start with an untitled document. Find highlights a match and wraps at the end of the document. The title shows unsaved changes. Open keeps existing drafts in tabs. Recent files are available across sessions. Rust, Python and JSON have syntax highlighting. Drafts are checkpointed for recovery if the editor exits unexpectedly. Closing an edited tab offers Save, Discard or Cancel. Closing the window reviews every unsaved document. If a file changes on disk, Save As lets you keep your edits in another file. [Text Editor tests](docs/editor-tests.md)

| Action | Shortcut |
| --- | --- |
| New tab / previous or next tab | Ctrl+N or Ctrl+T / Ctrl+Page Up or Ctrl+Page Down |
| Recent files / find and replace | Ctrl+P / Ctrl+H |
| Go to a numbered line | Ctrl+I, enter the line number, then Enter |
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
| Filter by name or PID / show resource history | Ctrl+F / Ctrl+R |
| Sort by process name, PID, CPU or memory | N, P, C or M |
| Reverse the current sort | Activate the same column again |
| Select a process | ↑ / ↓, Home or End |
| Request termination / cancel | Delete / Escape |
| Increase / decrease / reset row text size | Ctrl++ / Ctrl+- / Ctrl+0 |

## Image Viewer

Open a local PNG, JPEG, WebP or standalone SVG from Files, the command line, or the **Open** button. The viewer applies embedded orientation, fits the whole image to the window, and shows its upright dimensions. Zoom is relative to that fit. A failed Open leaves the current image available. [Image Viewer tests](docs/viewer-tests.md)

| Action | Shortcut |
| --- | --- |
| Open / close | Ctrl+O / Ctrl+W |
| Zoom in / out | + / − (also Ctrl+= / Ctrl+-) |
| Previous / next image in the folder | Page Up / Page Down |
| Pan a zoomed image | Arrow keys or scroll |
| Fit to window | F or Ctrl+0 |
| Reach Open / return to the image | F6 / Escape |

## Calculator

Use % for percentages, including relative increases and discounts such as `120 + 15%`. Choose **Units** (Ctrl+U) or type a conversion directly, for example `32 °F to °C`. Results can be reused at full precision.

Enter an arithmetic expression or use the keypad. Calculator handles parentheses and decimal arithmetic exactly. It displays repeating decimals as reduced fractions. Choose **Use result** to continue a calculation or select an earlier history row to reuse its answer. The window keeps the latest 50 successful calculations. **Clear history** leaves the current expression and result visible. [Calculator tests](docs/calculator-tests.md)

| Action | Shortcut |
| --- | --- |
| Evaluate the expression | Enter |
| Use the displayed result | Ctrl+R |
| Reach the newest history entry | Alt+H |
| Clear the expression / close Calculator | Ctrl+L / Ctrl+W |
| Reach the header / return to the expression | F6 / Escape |

## Packages

Open **Discover** (Ctrl+D) to browse applications by category and launch installed apps. Use **Installed** to browse local packages, or search for a name or description. Open a result to inspect dependencies and review installation or removal. **Updates** shows the running deployment, the next boot, and upgrades prepared for shutdown. Each change requires confirmation of a specific dave plan. [Packages tests and polkit contract](docs/packages-tests.md)

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
| Toggle continuous scrolling / open the outline | Ctrl+J / Ctrl+B |
| Select and copy page text | Drag or Shift+Left/Right, then Ctrl+C |
| Print | Ctrl+P |
| Enter a page number | Ctrl+L, then Enter |
| Find text | Ctrl+F, type a phrase, then Enter |
| Next / previous match | F3 / Shift+F3 |
| Zoom in / out / return to 100% | Ctrl+= / Ctrl+- / Ctrl+0 |
| Scroll / leave Find | Arrow keys on the page / Escape |

## Video

Video plays local WebM (VP8 or VP9) and MP4 (H.264). Soundtracks can use AAC, Opus or Vorbis. AAC uses a decoder from the system GStreamer installation. Sound goes directly to PipeWire. Click or drag the seek slider, use the five-second step buttons, or operate the player by keyboard. At the end of a clip, **Replay** starts it again. [GStreamer packaging list and Video tests](docs/video-tests.md)

| Action | Shortcut |
| --- | --- |
| Open / close | Ctrl+O / Ctrl+W |
| Enter fullscreen / leave fullscreen | F11 / Escape |
| Show controls in fullscreen | Click the picture or press F6 |
| Play or pause with the picture focused | Space |
| Step back / forward five seconds | Left / Right |
| Seek to start / end | Home / End |
| Reach Open / return to the picture | F6 / Escape |

## Music

Music plays local MP3, M4A/AAC, FLAC, PCM WAV, Ogg Opus and Ogg Vorbis through PipeWire. Seek on the waveform or use Slower and Faster ([ and ]) to play speech at 0.5× through 2×. **Add tracks…** appends files to the queue. A single click selects an entry for reordering or removal. Double-click or choose **Play selected** to start it. The current track shows its title, artist and album when tagged. [Music packaging and tests](docs/music-tests.md)

| Action | Shortcut |
| --- | --- |
| Add tracks / close the window | Ctrl+O / Ctrl+W |
| Select a queue entry / play it | Up or Down / Enter |
| Move the selected entry / remove it | Ctrl+Up or Ctrl+Down / Delete |
| Previous or next track | P / N |
| Pause or resume with the queue focused | Space |
| Seek five seconds / seek to start or end | Left or Right / Home or End |
| Focus Add tracks / return to the queue | F6 / Escape |
| Increase, decrease or reset text size | Ctrl++ / Ctrl+- / Ctrl+0 |

## Archive Manager

Open a ZIP, tar or gzip-compressed tar archive with Ctrl+O. Use Up and Down to browse the tree and Space to mark entries. Ctrl+E extracts the marked entries or the focused entry, including a selected folder’s descendants. Ctrl+Shift+E extracts the whole archive. Choose a destination in the portal. Archive Manager creates a new folder there and preserves existing files. [Extraction rules and tests](docs/archive-tests.md)

## Disk Usage

Open a folder with Ctrl+O to see where its space goes. Enter or double-click opens a child folder. Backspace returns to its parent. Press A to switch between allocated space and logical file sizes, F5 to rescan, or Escape to cancel a scan. Unreadable folders stay visible and their totals are marked incomplete. [Accounting and tests](docs/usage-tests.md)

## Disks

Browse drives and partitions with Up and Down. Select a filesystem to see its mount state. The **Mount** or **Unmount** button, also available with Ctrl+M, asks UDisks to perform the operation under the system’s polkit policy. F5 refreshes the device list. [UDisks integration and tests](docs/disks-tests.md)

## Clocks

Use **Add city** (Ctrl+N) to find a world clock by city or IANA time zone. Open **Timer** (Ctrl+T) to start a countdown in seconds, m:ss or h:mm:ss. Escape returns to the saved cities. [Clocks tests](docs/clocks-tests.md)

## Characters

Search by Unicode name or code point (for example, U+1D11E). Enter focuses the results. Use the arrow keys to select a character, then Ctrl+C or **Copy character** to copy it. Ctrl+F returns to search. [Characters tests](docs/characters-tests.md)

## Fonts

Browse installed families with Up and Down, then use Left and Right to preview another face. Ctrl+F finds a family and Ctrl+E selects the sample for replacement. The status lists any characters missing from the selected face. [Fonts tests and dependencies](docs/fonts-tests.md)

## Weather

Choose a city or enter coordinates and an IANA time zone. Ctrl+L changes location and F5 checks the forecast. Weather keeps the last forecast available offline and shows its update time. [Weather provider and tests](docs/weather-tests.md)

## Maps

Ctrl+F focuses place search. Submit with Enter, then choose a result. Drag or use the arrow keys to pan, and use + and − to zoom. Ctrl+S saves the visible area at its current zoom. Ctrl+O opens the saved area or returns online. [Maps providers, limits and tests](docs/maps-tests.md)

## Building

Distribution builds can use the vendored source archive from `tools/dist.sh`. [Offline packaging instructions](docs/plan.md#vendored-source-archive)

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
./target/release/mundane-music tests/fixtures/music/01.flac tests/fixtures/music/02.opus
./target/release/mundane-archive-manager "tests/fixtures/archive/Coastal survey.zip"
./target/release/mundane-disk-usage tests/fixtures
./target/release/mundane-disks
./target/release/mundane-clocks
./target/release/mundane-characters
./target/release/mundane-fonts
./target/release/mundane-weather
./target/release/mundane-maps
```

Files uses the OpenURI desktop portal or GIO's default handler, GIO Trash, and GNU coreutils for recursive copies and cross-filesystem moves. Its desktop entry handles local directories. Text Editor accepts a local path and declares `text/plain`. Its Open and Save As dialogs use the FileChooser desktop portal. The joe package at `182cd7c` has been tested on GNOME Wayland. [Packaged-image smoke results](docs/vm/182cd7c-dave-r3/README.md) [Development plan](docs/plan.md)

## Licence

Original code and documentation use the [BSD 3-Clause licence](LICENSE). The shared text field adapts GPUI's text input example from Zed revision `deb48ceb47125223df73902d1c3b72f2f442222b`. That upstream code retains its [Apache 2.0 licence](LICENSES/Apache-2.0.txt). Fonts are resolved from the system.
