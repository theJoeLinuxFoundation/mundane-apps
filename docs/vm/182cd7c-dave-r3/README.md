# Packaged joe desktop smoke test with dave r3

All 18 apps launched, rendered and closed in joe’s GNOME Wayland session. All 27 launch cases passed. The eight file handlers opened their fixtures through **Open With**, Disk Usage scanned its folder and the final journal scan found no suite crashes. The two packaging failures from the previous image are resolved. [Full results](results.json) · [Previous run](../182cd7c/README.md) · [Harness](../../../tests/joe-vm/README.md)

## Package review and login alarms

- **Packages** displayed the installed package list. Its first page matched dave’s inventory of 449 packages. From the search result, the test opened the installed package’s details and its removal review. The dialog showed **Remove joe-test-desktop · 0.1-r1 · Selected** and focused Cancel. The unprivileged dry run returned a `dave.plan/1` digest of `sha256:59c2e08e115e72a40686b36ca45303666d0153076c11980b5f297e19008eaac5`. Escape dismissed the dialog and the installed inventory digest matched afterward. The image includes dave’s polkit policy. No transaction was authorized. [Installed list](screenshots/packages-desktop.png) · [Review dialog](screenshots/packages-review.png)
- **Clocks** started its watcher on login. The test saved a future alarm in the overlay, restarted GDM and waited for joe’s next automatic GNOME login. Before any suite window opened, it found `/usr/bin/mundane-clocks --watch-alarms` as UID 1000, PID 1286, in the GNOME Clocks alarm autostart scope. The Clocks window subsequently opened and closed cleanly. [Clocks window](screenshots/clocks-desktop.png)

## Desktop file defaults

Fresh `gio mime` processes queried all 28 declared types under each desktop identity. The new profile selected a suite handler for **28 of 28** under `XDG_CURRENT_DESKTOP=mundane`. All 28 GNOME choices and all 28 choices with the variable unset matched measurements on the previous frozen image. [Per-type results](results.json) · [Prior-image baseline](../../../tests/joe-vm/gnome-defaults.json)

GNOME selects a GNOME app for 27 types. Generic `audio/ogg` already selected mundane Music on the previous image and still does. The specific Opus and Vorbis types select Decibels. With the variable unset, GIO already selected mundane Music for `audio/flac`, `audio/mp4`, `audio/ogg`, `audio/x-flac`, `audio/x-m4a`. These are existing defaults to account for if the distribution wants GNOME handlers for every type outside mundaneDE.

## Per-app evidence

The window and fixture links are QMP screenshots. Every case passed its foreground-window, painted-content and process-exit checks. **Clear** records a crash-free journal check. All 18 launcher icons resolved in Papirus. Maps and Weather ran offline.

| App | Desktop launch and render | Additional open or action | Closed | Crash journal |
| --- | --- | --- | --- | --- |
| Archive Manager | [Window](screenshots/archive-manager-desktop.png) | Open With · [Coastal survey.zip](screenshots/archive-manager-mime.png) | Yes | Clear |
| Calculator | [Window](screenshots/calculator-desktop.png) | No file association | Yes | Clear |
| Characters | [Window](screenshots/characters-desktop.png) | No file association | Yes | Clear |
| Clocks | [Window](screenshots/clocks-desktop.png) | No file association | Yes | Clear |
| Disk Usage | [Window](screenshots/disk-usage-desktop.png) | Folder argument · [Smoke folder](screenshots/disk-usage-folder.png) | Yes | Clear |
| Disks | [Window](screenshots/disks-desktop.png) | No file association | Yes | Clear |
| Document Viewer | [Window](screenshots/document-viewer-desktop.png) | Open With · [Coastal survey.pdf](screenshots/document-viewer-mime.png) | Yes | Clear |
| Files | [Window](screenshots/files-desktop.png) | Open With · [Smoke folder](screenshots/files-mime.png) | Yes | Clear |
| Fonts | [Window](screenshots/fonts-desktop.png) | Open With · [Survey.ttc](screenshots/fonts-mime.png) | Yes | Clear |
| Image Viewer | [Window](screenshots/image-viewer-desktop.png) | Open With · [North Point.jpg](screenshots/image-viewer-mime.png) | Yes | Clear |
| Maps | [Window](screenshots/maps-desktop.png) | No file association | Yes | Clear |
| Music | [Window](screenshots/music-desktop.png) | Open With · [02.opus](screenshots/music-mime.png) | Yes | Clear |
| Packages | [Window](screenshots/packages-desktop.png) | No file association · [Reviewed plan](screenshots/packages-review.png) | Yes | Clear |
| System Monitor | [Window](screenshots/system-monitor-desktop.png) | No file association | Yes | Clear |
| Terminal | [Window](screenshots/terminal-desktop.png) | No file association | Yes | Clear |
| Text Editor | [Window](screenshots/text-editor-desktop.png) | Open With · [Dispatch.txt](screenshots/text-editor-mime.png) | Yes | Clear |
| Video | [Window](screenshots/video-desktop.png) | Open With · [Coastal passage.webm](screenshots/video-mime.png) | Yes | Clear |
| Weather | [Window](screenshots/weather-desktop.png) | No file association | Yes | Clear |

## Image identity

The read-only backing was `joe-desktop-suite-182cd7c-dave-r3.img`, with suite revision `182cd7c` and dave `0.4.0-r3`. Its SHA-256 before and after testing was `69e8ac25679550141aa79485f7c8439bd38bfe0e380f9197d666c5261311515e`. All state changes went to the disposable qcow2 overlay. The harness revision was `0bcef7f`.
