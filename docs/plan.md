# Development plan

**Repository** — `theJoeLinuxFoundation/mundane-apps`. The prefix identifies the desktop these applications are designed for and makes them easy to find alongside mundaneDE. The target is mundaneDE on x86 joe Linux.

## Workspace and shared interface

Use one Cargo workspace and lockfile. Each application in `apps/` builds as an independently installable binary. Keep its data model, commands and format handling separate from its GPUI views so unit tests can exercise them directly. Introduce the following shared crates with Terminal and Files as their first consumers.

- **mundane-ui** owns semantic colours, type sizes, spacing, focus rings, keyboard actions and common controls. Start with window headers, buttons, text fields, lists, dialogs and status messages. Use the same selection, focus and error states throughout the suite. Windows participate in mundaneDE's normal placement and resizing.
- **mundane-services** provides small interfaces for the desktop facilities below. Give each application only the features it uses. Run blocking work off the UI thread and deliver results to the requesting view. A dismissed view must be able to cancel a scan or ignore a late reply.
- The settings panel stays in mundaneDE. Agree on theme roles and a sample settings row with its maintainers through the project coordinator. The panel can depend on `mundane-ui` at a pinned repository revision and supply its own settings model. Review the same sample in both projects when changing the theme API.

Milestones 1–4 used Zed revision `deb48ceb`. The workspace now pins GPUI and its Linux platform together at `f4fe0be1f8c7974db07e01c5cd05bee0cf4eb8d9`, which includes the upstream AccessKit bridge. Release dependencies enable the Wayland backend and its wgpu renderer with default platform features disabled. GPUI's headless test support has a separate feature set. [Pinned Linux platform manifest](https://github.com/zed-industries/zed/blob/f4fe0be1f8c7974db07e01c5cd05bee0cf4eb8d9/crates/gpui_linux/Cargo.toml) · [Pinned GPUI manifest](https://github.com/zed-industries/zed/blob/f4fe0be1f8c7974db07e01c5cd05bee0cf4eb8d9/crates/gpui/Cargo.toml)

## Desktop services

| Facility | Shared behaviour |
| --- | --- |
| D-Bus and portals | Use the session bus for portal file choosers, opening URIs and notifications. Handle cancellation and unavailable services as distinct outcomes. Use the system bus for UDisks operations and let polkit mediate privileged actions. |
| Launching and file types | Give each app a stable Wayland app ID matching the basename of its desktop entry. Declare supported MIME types and file or URI arguments in that entry. Files uses the user's preferred handler. Installing a viewer preserves existing user associations. |
| Thumbnails | Files requests visible items first. Use the freedesktop thumbnail cache under the XDG cache directory, check source URI and modification time, and bound decoding work. A missing or failed preview falls back to the MIME icon. |
| Preferences | Read desktop appearance from the settings portal where available. Store versioned per-app preferences under the XDG config directory, write atomically, and keep recoverable window state under the XDG state directory. Give defaults for missing keys and test migration from the previous format. |
| Fonts and icons | Resolve installed fonts through fontconfig with script and emoji fallback. Look up named icons in the system theme with a fallback. Test with the fonts and Papirus or Adwaita icons supplied by joe Linux. |
| Media and devices | Share playback controls across Video and Music. Use the system media stack for decoding and PipeWire for audio output. Disks identifies devices through UDisks object paths and handles devices disappearing during an operation. |

## Acceptance tests

Terminal, Files, Text Editor, System Monitor, Image Viewer, Calculator and Document Viewer have passed their model or service tests, GPUI headless tests and software-rendered Wayland acceptance tests. The shared header has also been exercised with client and server decorations. The shared accessibility contract is implemented. Three release assertions remain open. [Accessibility report](accessibility/report.md) Packages is waiting for dave’s machine-readable package interface. The target-image smoke tests are pending until joe Linux packages mundaneDE and the suite together, when they will run in joe’s VM tests. [Terminal report](terminal-tests.md) · [Files report](files-tests.md) · [Text Editor report](editor-tests.md) · [System Monitor report](monitor-tests.md) · [Image Viewer report](viewer-tests.md) · [Calculator report](calculator-tests.md) · [Document Viewer report](document-tests.md)

Each numbered row is a milestone in delivery order. For every row, run its model or service tests, drive its user interaction in a GPUI headless test context, then capture the specified state from the actual Wayland client under a nested or headless compositor. The headless interaction test asserts state and focus. The compositor test checks the rendered result against a reviewed reference image.

Use temporary XDG directories, private fixture data, fixed fonts, locale, time zone and window size. Inject the clock, service responses and media position where needed. Use a fixed Mesa renderer for visual comparisons and smoke-test launch, resize, input and close in mundaneDE on the target image. A changed screenshot requires review against the intended interaction.

| Order and app | First usable milestone | Unit or service test | GPUI headless interaction | Wayland screenshot |
| --- | --- | --- | --- | --- |
| 1. Terminal | Run the user's shell in a PTY with resize, selection and scrollback. | Replay escape sequences and wide glyphs into the grid. Verify a PTY child sees the new size and its exit is reaped. | Type a command, select its output and copy it. | Resized grid with selected text and a wide glyph. |
| 2. Files | Browse directories, open files with the preferred handler, copy, move, rename and trash local items. | Exercise name collisions, dangling links and denied writes in a fixture tree. | Navigate by keyboard, rename an item and cancel a copy conflict. | Directory listing with a thumbnail and a focused conflict dialog. |
| 3. Text Editor | Open, edit, find and save UTF-8 documents. Confirm closing unsaved work. | Round-trip CRLF and Unicode. A failed save preserves the original. | Edit, find the edit, dismiss the close prompt and save. | Search match with the dirty document indicator visible. |
| 4. System Monitor | Inspect per-process CPU and memory, sort processes and signal a selected process. | Compute deltas from procfs samples including an exited process and a reused PID. | Sort by CPU and confirm termination against a fake process service. | Sorted list with the selected process and its memory use. |
| 5. Image Viewer | Open PNG and JPEG, respect orientation, zoom and fit to the window. | Check rotated dimensions and reject a truncated image. | Open a fixture, zoom in and return to fit. | Known oriented image fitted within the window. |
| 6. Calculator | Evaluate arithmetic expressions, reuse results and clear history. | Check precedence, decimal arithmetic and division by zero. | Enter an expression by keyboard and reuse its result. | Expression, result and history at a narrow width. |
| 7. Packages (dave) | Search, inspect, preview and confirm installation or removal, and show upgrade status. Requires the dave interface below. | Decode versioned fixtures including failed operations and unexpected fields. | Search, inspect dependencies, cancel once, then confirm a fake transaction exactly once. | Dependency changes awaiting confirmation. |
| 8. Document Viewer | Read PDFs with page navigation, search and zoom. | Check page bounds, search hits and an unreadable document. | Jump to a page, find text and zoom. | Second page with a search match. |
| 9. Video | Play local video with seek, pause and audio output. | Map duration to seek positions and report an unavailable decoder. | Open a clip, seek and pause playback. | Known paused frame with elapsed time. |
| 10. Music | Play a queue of local audio tracks and display track metadata. | Advance at end of stream and fall back to a filename for missing tags. | Reorder the queue, skip a track and pause. | Queue with the current track selected. |
| 11. Archive Manager | List and extract ZIP and tar archives to a chosen directory. | Reject paths and links that would escape the destination. | Select entries, choose a destination and inspect the extraction result. | Archive tree with a nested entry selected. |
| 12. Disk Usage | Scan a directory, show sizes by child and drill down. | Count hard links once and finish a scan with unreadable entries. | Drill down, navigate back and cancel a scan. | Sorted sizes with an unreadable directory marked. |
| 13. Disks | Inspect drives and partitions and mount or unmount a volume through UDisks. | Handle hot removal and a denied mount on a fake bus. | Select a test volume, mount it and observe the updated action. | Drive layout with a mounted partition. |
| 14. Clocks | Show world clocks and run a countdown timer. | Cover a daylight-saving transition and timer expiry with a fake clock. | Add a city, start a timer and dismiss its notification. | Two cities at a fixed instant. |
| 15. Characters | Find Unicode characters by name or code point and copy them. | Match a supplementary-plane code point and its name. | Search by code point and copy the selected character. | Selected glyph with its name and code point. |
| 16. Fonts | Browse installed font families and preview a face with sample text. | Enumerate a fixture family with two faces and identify missing glyphs. | Switch faces and replace the sample text. | Multiscript sample with the selected face visible. |
| 17. Weather | Choose a location, show its forecast and identify stale data offline. | Check forecast units, time zone and cache expiry with a fixture provider. | Select a city and reopen its cached forecast offline. | Forecast with its last update time and offline status. |
| 18. Maps | Find a place, pan and zoom the map, and display provider attribution. | Check coordinate bounds, tile selection and missing tiles with a fixture source. | Select a search result, pan and zoom by keyboard. | Selected place with legible attribution. |

A later Files polish pass will add Replace to the name-conflict dialog and show the size and modification date of both items.

The application inventory follows joe Linux's `packages/joe-desktop/package.zon`. For Weather and Maps, settle provider terms, attribution and caching permissions before connecting a live service. Acceptance tests use redistributable local fixtures.

### dave interface dependency

The current joe Linux sources (`dave/share/dave.8` and `dave/src/cli.zig`) scope `--json` to `install-system`, the OS installer. The Packages milestone needs a separate machine-readable contract for package operations. Coordinate with dave's maintainer on schemas for search, list, info, transaction previews, results and deployment status. Exercise them against the packaged CLI in a disposable joe Linux image before enabling those actions.

Run queries as the desktop user. Pass arguments directly to dave, show the full dependency change before confirmation, and authorize only the confirmed mutation through a narrow polkit helper. Distinguish an upgrade prepared for shutdown from the running deployment. Test authorization denial and a changed transaction preview between confirmation and execution. Keep mutations unavailable until the CLI contract can enforce the reviewed transaction.

## Accessibility

Use the keyboard to reach and operate every action. Give controls accessible names, roles and state in their shared definitions. Keep a visible focus indicator, return focus to the invoking control after a dialog closes, and label errors and selections with text or shape as well as colour. Test the primary workflow of each app by keyboard, including Escape and reverse tab order.

The migrated lockfile resolves each GPUI crate once at `f4fe0be1` in Cargo metadata for x86-64 Linux. Files and Text Editor now exercise focused buttons with a matching Enter press and release. All 17 workspace tests passed. The renderer changed window-control strokes and edge colors in 19 references. After review, all 26 captures matched the reference dimensions and RGBA pixels exactly on headless Sway and labwc.

The four apps now expose the shared controls through AccessKit. The implementation, live AT-SPI results, Orca transcript and measured cost are recorded in the accessibility report. The milestone remains open on disabled-button state, Find character echo and cached-subtree preservation. [Acceptance report](accessibility/report.md) · [Migration notes and gates at 52a66ec](https://github.com/theJoeLinuxFoundation/gpui-shades/blob/52a66ec/docs/linux-accessibility.md)

In the first Terminal milestone, probe the pinned GPUI Linux platform for a route to AT-SPI. Try a named button, an editable field and a dialog in an AT-SPI inspector and with Orca. Verify focus, text, role and activation. If a platform bridge is needed, record the missing pieces and track bridge work with mundaneDE before claiming screen-reader support. Test high contrast, 100%, 150% and 200% output scaling, and increased text size for clipping, readable focus and reachable dialog actions.

## Dependency licences

Choose permissive engines for the suite. GPL and AGPL libraries are excluded from static and dynamic linking. LGPL system libraries may be linked dynamically. The approved compiler-runtime exception covers `libgcc_s` under the GCC Runtime Library Exception. Running a GPL application helper as a separate process requires the project owner’s prior approval. If a feature needs a GPL component, bring that decision to the owner before implementing it.

The workspace test runs Cargo metadata for x86-64 Linux and checks normal dependencies reachable from each binary, including optional features. It evaluates SPDX choices and conjunctions, rejects missing or unreviewed terms and checks declared native link names. For example, `self_cell` offers Apache-2.0 as an alternative to GPL, and the existing `option-ext` dependency uses MPL-2.0. Review native build scripts, bundled components and runtime-loaded libraries separately. Record the chosen licence and distribution notices for each new engine. [Metadata test](../tests/dependency_licences.py) · [PDFium packaging and notices](document-tests.md)

## joe Linux packaging handoff

The joe Linux packaging maintainer will add recipes in the distribution. Supply a locked dependency graph for an offline build, a list of native build and runtime dependencies per binary, desktop entries, MIME declarations and licence notices. Build for x86 joe Linux with glibc. Verify launch on its Mesa stack in a systemd user session with D-Bus and the configured portal backend. Check audio through PipeWire and authorized device operations against polkit and UDisks where applicable.

Install each binary under `/usr/bin` and its desktop entry under `/usr/share/applications`. Test launching from the menu and opening a representative file through its registered handler on a clean image. Replace each GNOME application in the desktop package after its corresponding milestone passes on the image. Check reverse dependencies before retiring shared services such as GVfs.

Use system icon themes, fonts, sounds and codecs. Review the licence and provenance of every proposed bundled asset or fixture before adding it. Keep original project files under BSD-3-Clause, use BSD-compatible bundled assets and preserve required third-party licences and notices, including Apache-2.0 notices for any adapted GPUI code.
