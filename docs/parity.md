# Daily-use parity with GNOME

This inventory compares mundane-apps at `7854078` with the GNOME 51 desktop assembled by joe. Versions below come from joe’s package recipes at `5dc2ac8`. Several components have their own release series. joe packages GNOME Terminal 3.60.0, so that is the terminal baseline. Each version links to the upstream source archive selected by joe. The comparison uses those archives’ help, metainfo and UI sources. [joe package recipes](https://github.com/theJoeLinuxFoundation/joe-linux/tree/5dc2ac8f460e56b51a887cc752c101adc42daa30/packages) · [GNOME 51 release notes](https://release.gnome.org/51/)

The rankings estimate how often a person encounters a task while using that app. They are editorial judgments, without usage telemetry. Read each row left to right and the numbered items in order. This round covers Files navigation and filename search, Terminal tabs and web links, and Text Editor tabs and line navigation. The table records the starting point. The delivery notes below it mark what shipped and list the next gaps.

| Suite app and reference | Already available | Most often missed | Regular use | Occasional or specialist use |
| --- | --- | --- | --- | --- |
| **Files** · [Nautilus 51.1](https://download.gnome.org/sources/nautilus/51/nautilus-51.1.tar.xz) | Local browsing, thumbnails, standard file operations and Ctrl+H already work. | 1. Jump between Home, user folders, bookmarks and mounted volumes. 2. Find a file by name below the current folder. 3. Keep two folders open in tabs. | Select multiple items, drag and drop, switch list/grid views, choose an app with Open With, and inspect size, dates and permissions in Properties. | Mount or eject removable media, browse remote shares and search indexed contents. |
| **Terminal** · [GNOME Terminal 3.60.0](https://gitlab.gnome.org/GNOME/gnome-terminal/-/archive/3.60.0/gnome-terminal-3.60.0.tar.gz) | A real shell PTY, scrollback, keyboard copy/paste, Find and font zoom are present. | 1. Keep independent shells in tabs, open a tab in the working directory and switch back without losing the running job. 2. Open a web link printed by a command. | Choose a persistent font, scrollback size and color profile. Open a separate window from the running terminal. | Per-profile startup commands and login shells, detachable tabs and custom shortcuts. |
| **Text Editor** · [GNOME Text Editor 51.0](https://download.gnome.org/sources/gnome-text-editor/51/gnome-text-editor-51.0.tar.xz) | UTF-8 editing, undo, Find, zoom, CRLF preservation and guarded local saves are present. | 1. Keep multiple drafts and files open in tabs and close them safely. 2. Find a reported line by number and see line numbers in the document. 3. Reopen a recent file. | Syntax highlighting and indentation for source files, search and replace, and automatic draft saving between sessions. | Recover after an unexpected exit, reopen closed tabs and print. |
| **System Monitor** · [GNOME System Monitor 51.1](https://download.gnome.org/sources/gnome-system-monitor/51/gnome-system-monitor-51.1.tar.xz) | Process list, four sort keys and confirmed SIGTERM with a reused-PID guard. | Search/filter processes and see CPU, memory and network graphs over time. | Force-quit a stuck process and inspect filesystem capacity or process properties. | Change priority, stop/resume, inspect open files and memory maps. |
| **Image Viewer** · [Loupe 51.2](https://download.gnome.org/sources/loupe/51/loupe-51.2.tar.xz) | Oriented PNG/JPEG images, pan, fit and zoom. | Previous/next image in a folder and support for more common formats including WebP and SVG. | Rotate or crop, trash an unwanted image and inspect dimensions and EXIF details together. | Inspect the embedded color profile and camera/lens metadata. |
| **Document Viewer** · [Papers 51.1](https://download.gnome.org/sources/papers/51/papers-51.1.tar.xz) | PDF pages, embedded-text search, jump to page and zoom. | Continuous scrolling across page boundaries, select/copy text, print and navigate by thumbnails or outline. | Remember reading position, follow links, fill PDF forms and open password-protected documents. | Annotate, sign visually or open DjVu, TIFF and comic archives. |
| **Video** · [Showtime 49.1](https://download.gnome.org/sources/showtime/49/showtime-49.1.tar.xz) | Local WebM, playback position and seeking, with PipeWire audio. | Open common MP4/H.264 media where packaging permits, and enter fullscreen. | Select subtitles/audio language and adjust playback speed. | Capture a frame. Codec and plugin licensing needs a separate packaging review. |
| **Music** · [Decibels 49.6.1](https://download.gnome.org/sources/decibels/49/decibels-49.6.1.tar.xz) | Local playback, transport, metadata and an editable queue. | Seek visually with a waveform. Adjust speed for speech recordings. | Open additional common audio types available through the distribution’s GStreamer plugins. | Listen to multiple files concurrently in separate players. |
| **Calculator** · [GNOME Calculator 51.1](https://download.gnome.org/sources/gnome-calculator/51/gnome-calculator-51.1.tar.xz) | Exact basic arithmetic, decimal input and reusable calculation history. | Calculate percentages and convert common units without rewriting the expression. | Scientific functions, powers and configurable rounding or scientific notation. | Financial functions, currency rates, programming bases and bitwise operations. |
| **Clocks** · [GNOME Clocks 51.0](https://download.gnome.org/sources/gnome-clocks/51/gnome-clocks-51.0.tar.xz) | Saved world clocks and a single countdown with an expiry notification. | Create named alarms with repeat days and snooze. | Run a stopwatch with laps and keep more than one timer. | Show sunrise and sunset for a world clock. |
| **Weather** · [GNOME Weather 48.0](https://download.gnome.org/sources/gnome-weather/48/gnome-weather-48.0.tar.xz) | Choose a place or coordinates, inspect forecast periods and use a per-location cached forecast. | See a multi-day forecast summarized by day and switch quickly among saved locations. | Find more cities by name and use the current location with permission. | Desktop search integration for a place’s forecast. |
| **Maps** · [GNOME Maps 51.2](https://download.gnome.org/sources/gnome-maps/51/gnome-maps-51.2.tar.xz) | Vector map, explicit place search, pan/zoom and one saved offline area. | Get directions between places, see the current location and keep favourite places. | Inspect place details and manage multiple offline regions. | Transit departures and delays where supported, or import a route/shape file. Routing and transit need appropriate providers. |
| **Characters** · [GNOME Characters 51.0](https://download.gnome.org/sources/gnome-characters/51/gnome-characters-51.0.tar.xz) | Search by Unicode name or code point, inspect and copy a scalar. | Browse emoji and character categories, then return to recently used characters. | Find emoji by familiar keywords and select multi-code-point emoji. | Follow “See Also” to related Unicode characters. |
| **Fonts** · [GNOME Font Viewer 50.0](https://download.gnome.org/sources/gnome-font-viewer/50/gnome-font-viewer-50.0.tar.xz) | Browse installed families/faces and test glyph coverage with a custom sample. | Preview a downloaded font file and install it for the current user. | Compare several sizes and browse visual thumbnails of families. | Inspect font metadata including licence and designer. |
| **Disks** · [GNOME Disks 46.1](https://download.gnome.org/sources/gnome-disk-utility/46/gnome-disk-utility-46.1.tar.xz) | List UDisks drives/partitions, mount or unmount, and track device changes. | Identify free space and filesystems visually, safely power off/eject a drive and unlock encrypted media. | Format a removable drive, create/delete partitions and edit mount options. | Inspect SMART health, benchmark a drive and create/restore disk images. |
| **Disk Usage** · [Baobab 50.0](https://download.gnome.org/sources/baobab/50/baobab-50.0.tar.xz) | Scan a local tree, compare allocated/logical size and drill into folders. | Find large subtrees in a rings/treemap view and open the folder in Files. | Choose a volume from an overview and trash a selected space hog. | Scan remote folders and revisit previous scans. |
| **Archive Manager** · [File Roller 44.7](https://download.gnome.org/sources/file-roller/44/file-roller-44.7.tar.xz) | List ZIP/tar/tar.gz and safely extract a selection or the entire archive. | Create an archive from files, add/remove entries, and open more common formats such as tar.xz. | Preview/open an entry and choose how to handle existing extraction targets. | Open encrypted archives, create password-protected archives and test integrity. |
| **Packages** · [GNOME Software 51.1](https://download.gnome.org/sources/gnome-software/51/gnome-software-51.1.tar.xz) | Installed packages, explicit search, dependency details and digest-bound install/remove/upgrade review. | Discover applications by purpose or category, see a human-readable description and screenshots, and launch an installed app. | See update details and notifications, application licence and source information. | Explain permissions or end-of-life status where metadata supports them. AppStream/catalog work and dave backend support must be assessed separately. |

## Files delivery

Files has a places sidebar with Home, configured XDG user folders, local GTK bookmarks and mounted volumes under `/media`, `/run/media` and `/mnt`. Bookmark folder and Ctrl+D toggle the current folder. F5 refreshes places and F9 toggles the sidebar. Ctrl+F searches filenames recursively on the current filesystem. Results include their parent folder. Escape ends the search. Hidden-file visibility applies to the whole search, symbolic-link directories are skipped, and the view reports unreadable folders and the 1,000-result limit.

| Workflow | Before | After |
| --- | --- | --- |
| Jump to a saved place | [Original folder](parity/before/files-places.png) | [Places and bookmarks](parity/after/files-places.png) |
| Find notes in a subfolder | [Original listing](parity/before/files-search.png) | [Filename results](parity/after/files-search.png) |

The next Files gaps are multi-folder tabs, multiselection, drag and drop, alternate views, Open With, Properties, and unmounted media.

## Terminal delivery

Each tab runs its own PTY and keeps its scrollback. New tab starts in the active shell’s directory when Linux exposes it. Ctrl+Shift+T opens a tab, Ctrl+Page Up/Down switches, and Ctrl+Shift+W closes the current tab. An exiting shell closes its own tab. Ctrl+click opens a printed HTTP(S) URL or an OSC 8 web link in the default browser. Find or select a URL and press Ctrl+Enter to open it from the keyboard.

| Workflow | Before | After |
| --- | --- | --- |
| Start a second shell in the working directory | [Single shell](parity/before/terminal-shells.png) | [Two shell tabs](parity/after/terminal-shells.png) |

Round one left persistent profiles, new-window controls, configurable shortcuts and running-job warnings for later work.

## Text Editor delivery

Open a second file or use New tab for another draft. Each tab keeps its text, selection, scroll position and undo history. Opening a file already in a tab returns to that tab. Ctrl+W closes the active document. Window close asks about unsaved documents, including those in background tabs. The gutter numbers logical lines. Ctrl+I or **Go to line** moves the caret to a one-based line number. An out-of-range entry shows the document’s valid range.

| Workflow | Before | After |
| --- | --- | --- |
| Keep two documents open and return to line four of the first | [Single document](parity/before/text-editor-documents.png) | [Tabs, gutter and caret](parity/after/text-editor-documents.png) |
| Enter a line number | [Original editor](parity/before/text-editor-go-to-line.png) | [Go to line](parity/after/text-editor-go-to-line.png) |

Round one left recent files, syntax highlighting, indentation settings, search and replace, autosave and crash recovery for later work.

## Reproduce the comparisons

Build the three apps with `cargo build --locked -p mundane-files -p mundane-terminal -p mundane-text-editor`. Run `tests/wayland/run --app files --parity after` and repeat with `terminal` and `text-editor`. The runner uses a disposable home, private D-Bus and headless Sway with software rendering. The tests compare pixels and exercise the workflows. For the before images, use the same runner with `--parity before` and set the corresponding `MUNDANE_FILES_BIN`, `MUNDANE_TERMINAL_BIN` or `MUNDANE_TEXT_EDITOR_BIN` to a binary built from `7854078`. [Wayland test dependencies](terminal-tests.md#software-rendered-wayland-tests)

## Round two · Text Editor

**Recent** (Ctrl+P) lists the last 20 local files opened or saved, most recent first. Ctrl+H opens literal, case-sensitive Find and replace. Tab moves from the query to the replacement. Replace all is one undo step. Rust, Python and JSON files use the MIT-licensed Tree-sitter highlighter and grammars. Other files retain plain text. [Tree-sitter](https://github.com/tree-sitter/tree-sitter)

While editing, the app checkpoints dirty tabs in private files under `$XDG_STATE_HOME/mundane-text-editor` (normally `~/.local/state/mundane-text-editor`). A restarted editor restores abandoned drafts, skips drafts owned by a live instance, and preserves the original file version for conflict detection. Save and Discard retire the recovery copy. Edits since the last checkpoint can be lost in a crash. Indentation settings, reopening closed tabs and printing remain open.

| Workflow | Before | After |
| --- | --- | --- |
| Open a recent file | [Original window](parity/round-2/before/text-editor-recent.png) | [Recent files](parity/round-2/after/text-editor-recent.png) |
| Read Python | [Plain text](parity/round-2/before/text-editor-syntax.png) | [Syntax highlighting](parity/round-2/after/text-editor-syntax.png) |
| Replace a name | [Find](parity/round-2/before/text-editor-replace.png) | [Find and replace](parity/round-2/after/text-editor-replace.png) |
| Restart after a forced exit | [Empty document](parity/round-2/before/text-editor-recovery.png) | [Restored draft](parity/round-2/after/text-editor-recovery.png) |

Use `tests/wayland/run --app text-editor --round-two after` to replay this round. The before variant uses a binary built from `f9b9512`.

## Round two · Terminal

**Profiles** (Ctrl+Shift+P) saves named font families, sizes, scrollback limits and Desktop, Light or Dark colors in the user’s configuration directory. Save and use applies to the current tab and sets the default for future shells. Existing tabs keep their own profile and zoom. Desktop high contrast takes precedence. To create another profile, enter a new name. To remove one, select it and choose Delete.

Closing a tab checks its PTY foreground process group and the shell’s live children, including background and stopped jobs. Closing the window checks every tab. Cancel keeps the shells running. New-window controls, detachable tabs, custom shortcuts and per-profile startup commands remain open.

| Workflow | Before | After |
| --- | --- | --- |
| Configure a shell | [Original terminal](parity/round-2/before/terminal-profiles.png) | [Profile settings](parity/round-2/after/terminal-profiles.png) |
| Close a tab running sleep | [Running job](parity/round-2/before/terminal-running-job.png) | [Cancel or close](parity/round-2/after/terminal-running-job.png) |

## Round two · Files

New tab (Ctrl+T) opens the current folder. Ctrl+Page Up/Down switches tabs and Ctrl+W closes the active tab. Each tab remembers its folder, hidden-file setting, search and selected path. Returning to a tab rereads the folder or reruns the search, so changes made in a sibling tab are visible. Multiselection and drag and drop are the next regular-use gaps, followed by alternate views, Open With and Properties.

| Workflow | Before | After |
| --- | --- | --- |
| Open a subfolder alongside its parent | [Single folder](parity/round-2/before/files-tabs.png) | [Folder tabs](parity/round-2/after/files-tabs.png) |
