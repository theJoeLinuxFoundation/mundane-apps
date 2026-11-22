# joe desktop image smoke test · 182cd7c

On 22 November 2026, all 18 installed apps launched from their desktop entries in joe’s GNOME Wayland session, painted a window, and exited after Alt+F4. All eight registered file handlers opened their fixtures through GNOME’s **Open With** path. Disk Usage also scanned its folder fixture. The journal contained no app crashes across the 27 launches. Two package integration checks failed. [Machine-readable results](results.json) · [Reproduce the run](../../../tests/joe-vm/README.md)

The [dave r3 image rerun](../182cd7c-dave-r3/README.md) verifies the package review, alarm watcher at login and desktop-specific MIME defaults.

## Changes for joe packaging

- **Packages needs dave 0.4.0-r3 or later with the JSON interface and polkit action.** The image contains `0.4.0-r2`. As joe, `/usr/bin/dave list --json` exited 1 with empty stdout and reported that `list` does not take `--json`. Packages displayed the resulting error instead of the installed list. The image also lacks `/usr/share/polkit-1/actions/io.joelinux.dave.policy`. Require the JSON-capable package when composing the image. The smoke run made no package changes. [Packages screenshot](screenshots/packages-desktop.png)
- **Install the Clocks alarm watcher’s login entry.** The window, world clocks and controls rendered. The package omits `/etc/xdg/autostart/org.joelinux.mundane.Clocks.Alarms.desktop`, which Clocks uses to resume scheduled alarms at login. Install the supplied entry at that path. [Source entry](../../../apps/clocks/data/autostart/org.joelinux.mundane.Clocks.Alarms.desktop)
- **Set the file defaults in the desktop profile that should use the suite.** All eight tested MIME registrations worked. This GNOME profile selected GNOME apps by default for every fixture. The table below records the actual associations. The test preserved them.
- **All 18 desktop icons resolved in Papirus**, the GNOME icon theme selected in this image. The GTK probe reported Adwaita as its toolkit theme. Its lookup missed `document-viewer`, `image-viewer`, `multimedia-audio-player` and `utilities-system-monitor`. A profile targeting Adwaita would need icons for those names. The GNOME launchers on this image need no icon change.

The tested PDFium page, JPEG, archive, font collection, WebM frame and Opus playback all loaded. Disks enumerated the virtual drive over UDisks. Maps displayed its offline state and Weather displayed its location chooser with the guest network disconnected.

## Per-app results

Each window link is a full-resolution QMP screendump. **Clear** means neither the per-launch journal interval nor the final boot-journal scan contained a crash for that application. The matching journal messages were Mesa EGL software-rendering warnings in this headless VM. A close passed only after the process owning the focused window exited.

| App | Desktop launch | Fixture through installed entry | Rendered screenshot | Closed | Crash journal |
| --- | --- | --- | --- | --- | --- |
| Archive Manager | Yes | Open With · Coastal survey.zip | [Window](screenshots/archive-manager-desktop.png) · [Fixture](screenshots/archive-manager-mime.png) | Yes | Clear |
| Calculator | Yes | No file association | [Window](screenshots/calculator-desktop.png) | Yes | Clear |
| Characters | Yes | No file association | [Window](screenshots/characters-desktop.png) | Yes | Clear |
| Clocks | Yes | No file association | [Window](screenshots/clocks-desktop.png) | Yes | Clear |
| Disk Usage | Yes | Folder argument · Smoke folder | [Window](screenshots/disk-usage-desktop.png) · [Fixture](screenshots/disk-usage-folder.png) | Yes | Clear |
| Disks | Yes | No file association | [Window](screenshots/disks-desktop.png) | Yes | Clear |
| Document Viewer | Yes | Open With · Coastal survey.pdf | [Window](screenshots/document-viewer-desktop.png) · [Fixture](screenshots/document-viewer-mime.png) | Yes | Clear |
| Files | Yes | Open With · Smoke folder | [Window](screenshots/files-desktop.png) · [Fixture](screenshots/files-mime.png) | Yes | Clear |
| Fonts | Yes | Open With · Survey.ttc | [Window](screenshots/fonts-desktop.png) · [Fixture](screenshots/fonts-mime.png) | Yes | Clear |
| Image Viewer | Yes | Open With · North Point.jpg | [Window](screenshots/image-viewer-desktop.png) · [Fixture](screenshots/image-viewer-mime.png) | Yes | Clear |
| Maps | Yes | No file association | [Window](screenshots/maps-desktop.png) | Yes | Clear |
| Music | Yes | Open With · 02.opus | [Window](screenshots/music-desktop.png) · [Fixture](screenshots/music-mime.png) | Yes | Clear |
| Packages | Yes | No file association | [Window](screenshots/packages-desktop.png) | Yes | Clear |
| System Monitor | Yes | No file association | [Window](screenshots/system-monitor-desktop.png) | Yes | Clear |
| Terminal | Yes | No file association | [Window](screenshots/terminal-desktop.png) | Yes | Clear |
| Text Editor | Yes | Open With · Dispatch.txt | [Window](screenshots/text-editor-desktop.png) · [Fixture](screenshots/text-editor-mime.png) | Yes | Clear |
| Video | Yes | Open With · Coastal passage.webm | [Window](screenshots/video-desktop.png) · [Fixture](screenshots/video-mime.png) | Yes | Clear |
| Weather | Yes | No file association | [Window](screenshots/weather-desktop.png) | Yes | Clear |

## Installed default handlers

For each fixture GIO detected the MIME type, found the suite’s entry among registered handlers, and opened the file with that entry. The resulting focused window belonged to the corresponding binary in `/usr/bin`. File-specific checks included document text, archive contents, track metadata and decoded image dimensions. Disk Usage takes a folder argument without claiming the directory MIME type.

| Fixture | Detected MIME type | Default desktop entry on this image |
| --- | --- | --- |
| Coastal survey.zip | `application/zip` | `org.gnome.Nautilus.desktop` |
| Coastal survey.pdf | `application/pdf` | `org.gnome.Evince.desktop` |
| Smoke folder | `inode/directory` | `org.gnome.Nautilus.desktop` |
| Survey.ttc | `font/collection` | `org.gnome.font-viewer.desktop` |
| North Point.jpg | `image/jpeg` | `org.gnome.Loupe.desktop` |
| 02.opus | `audio/x-opus+ogg` | `org.gnome.Decibels.desktop` |
| Dispatch.txt | `text/plain` | `org.gnome.TextEditor.desktop` |
| Coastal passage.webm | `video/webm` | `org.gnome.Showtime.desktop` |

## Run identity

| Item | Value |
| --- | --- |
| Frozen backing | `joe-desktop-suite-182cd7c.img`, raw, 18,255,708,160 bytes |
| Backing SHA-256 before and after testing | `b81856992d7d8f5d0d52d881feb2c5bd8f11e8aa1ebddb1580ba05befa5160c5` |
| Installed suite | `mundane-apps 0.1.0pre20261122-r1` built from `182cd7c` |
| Harness | `ea01d9e`, headless KVM, read-only backing nodes, fresh qcow2 overlay |
| Session | Autologged-in joe in GNOME on Wayland, serial root console used for orchestration |
| Outcome | 27 launches, 27 screenshots, 27 clean closes. 25 of 27 cases passed all checks. Packages and Clocks failed their package integration checks. |
