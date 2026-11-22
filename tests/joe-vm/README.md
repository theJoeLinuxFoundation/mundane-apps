# Packaged joe desktop smoke test

Run from the suite checkout on an x86 Linux host with KVM, QEMU, qemu-img, OVMF and ImageMagick. The image must be a standalone raw or qcow2 joe desktop test disk with the serial root prompt `joe# ` and automatic GNOME Wayland login for `joe`. The guest probes use its installed Python 3, PyGObject, GTK 4, GIO, AT-SPI, systemd journal and 9p filesystem support.

```sh
tests/joe-vm/run --list
python3 -B tests/joe-vm/test_harness.py
tests/joe-vm/run /path/to/joe-desktop-suite.img
```

Every invocation creates a new private directory under `target/joe-vm/`. QEMU uses KVM, virtio VGA, a virtual keyboard, silent guest audio and `-display none -nic none`. It receives a fresh qcow2 overlay, a private copy of the EFI variables and a read-only share of the guest probe and sample files. Both QEMU backing nodes are explicitly read-only and checked over QMP. Size, inode and backing timestamps are checked at shutdown. The guest user, desktop session and applications all come from the image. Set `OVMF_DIR` to the directory containing `OVMF_CODE.4m.fd` and `OVMF_VARS.4m.fd` when they are installed outside `/usr/share/edk2/x64`.

The runner waits for the serial root prompt, joe’s active logind Wayland session and GNOME Shell. It enables the accessibility bus and disables session idle timeout in the overlay. Desktop and MIME launches run as joe against that session. Each installed desktop entry is resolved with GIO, and its icon is checked against GNOME’s configured icon theme as well as the theme reported by GTK. The runner saves a future alarm in joe’s overlay home and restarts GDM. At the next automatic login it looks for joe’s actual `mundane-clocks --watch-alarms` process before launching any suite window. With an empty alarm list the watcher normally exits immediately.

For every declared MIME type, fresh `gio mime` processes resolve the default under `XDG_CURRENT_DESKTOP=mundane`, under `GNOME` and with the variable unset. The mundane profile is checked against the suite entries. The GNOME and unset profiles are compared with GIO measurements from the earlier frozen `182cd7c` image. [Original defaults](gnome-defaults.json)

In Packages, the runner reads the installed rows through AT-SPI, opens the installed package’s search result and details and opens **Review removal** for a requested package with a single-item dry-run plan. It checks the package, version, Apply button and initial Cancel focus, captures the dialog, dismisses it with Escape and verifies the installed inventory digest. The privileged transaction is outside this check.

| Check | Evidence |
| --- | --- |
| Desktop launch | Open each of the 18 installed desktop entries. Match the focused AT-SPI frame to the actual packaged executable. |
| MIME launch | Detect the fixture type with GIO. Record the installed default and registered handlers. Use `gio open` when the suite owns the default. Otherwise use the registered suite entry as in **Open With**. A missing registration is a failure even when passing the file directly to the entry succeeds. |
| Other inputs | Disk Usage accepts a folder through its entry. Nine utilities declare no file association. Their desktop case exercises their normal initial window. |
| Rendering | Match named controls and fixture contents in the foreground app, then compare the actual QMP scanout with the pre-launch desktop. PDF, JPEG and video also require colours from their decoded fixtures. The accepted scanout is saved as PNG. |
| Close and journal | Send Alt+F4 through QMP, wait for that exact process to exit, and read its journal from a per-case cursor. Check coredumps, kernel traps, panic and fatal exit messages. A final journal pass catches delayed records. Relevant journal messages are retained even when the crash check passes. |

The text and directory cases share a short dispatch. Other fixtures include a two-page coastal survey PDF, oriented JPEG, WebM clip, tagged Opus recording, ZIP and font collection. Maps and Weather are exercised offline. The guest keeps its installed MIME preferences for the entire run.

The runner prints the artifact directory. It contains `report.json`, screenshots named by app and launch mode, failure screendumps, the serial transcript and QEMU log. A nonzero exit reports a failed check or an incomplete run. A failed window check is recorded before cleanup targets processes started by that case. Pre-existing processes such as the Clocks alarm watcher are excluded.

The self-test boots firmware on a synthetic disk, verifies KVM and the read-only backing chain, injects a key, captures the scanout and checks that writing through the overlay preserves the original disk digest. It also checks coverage, journal attribution and render detection against the suite’s reviewed images.
