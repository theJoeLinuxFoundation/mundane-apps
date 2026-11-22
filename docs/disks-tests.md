# Disks and UDisks2

Disks reads `GetManagedObjects` from `org.freedesktop.UDisks2` on the system bus. It groups drives, partition tables and partitions by their D-Bus object paths, orders partitions by offset, and shows device names, capacities, filesystem types and mount points. Drive-less volumes appear at the top level. The view refreshes every two seconds, on F5, and after a mount or unmount request. [UDisks block interface](https://storaged.org/udisks/docs/gdbus-org.freedesktop.UDisks2.Block.html) · [Partition interface](https://storaged.org/udisks/docs/gdbus-org.freedesktop.UDisks2.Partition.html)

**Mount** and **Unmount** call the selected object's `org.freedesktop.UDisks2.Filesystem` method with an empty options dictionary and the D-Bus interactive-authorization flag. UDisks chooses the mount path and applies the system’s polkit policy. Ship the UDisks2 service, its distribution policy and a session authentication agent. The app runs as the desktop user. [Filesystem methods](https://storaged.org/udisks/docs/gdbus-org.freedesktop.UDisks2.Filesystem.html)

Before sending a change, the app re-reads the object and compares its daemon owner, object path, drive identity and detection time, device number, UUIDs and mount state. It addresses the method to that daemon’s unique bus name. Removal or replacement clears the selection. A refusal, dismissal or busy volume produces a status message and a fresh inventory. Disks sends each requested change once.

## Verification

The service and GPUI tests each start a disposable D-Bus daemon and a fake UDisks service. The service test covers polkit denial, a replacement at the same object path, disappearance during Mount, and a subsequent attempt against the removed object. The GPUI keyboard test selects a partition, mounts and unmounts it, then observes removal on the refresh timer. The fake records the method, object path, options and interactive-authorization flag. [Fake UDisks service](../tests/disks/udisks.py)

The Wayland harness routes the release binary’s system-bus connection to the private test daemon. Six pixel references show the mounted Survey partition on headless Sway and labwc. The profiles cover 100%, 150% and 200%, including larger text and high contrast. The private AT-SPI inspector checks the selected volume, mount point and Unmount action. The references and tests use synthetic device paths. Real polkit interaction and hardware integration remain pending on the packaged joe image.

```sh
cargo test --workspace --locked --offline
cargo build --release --locked --offline -p mundane-disks
export MUNDANE_DISKS_BIN="$PWD/target/release/mundane-disks"
for c in sway labwc; do
    for p in standard fractional contrast; do
        tests/wayland/run --app disks --compositor "$c" --profile "$p" || exit 1
    done
done
tests/wayland/run --app disks --at-spi
```

The fake requires dbus-daemon, Python and PyGObject with Gio. The tests need no administrator privileges.

Round four adds a partition layout and private-bus Eject, PowerOff and Unlock tests. The fake service checks the interactive authorization flag, records no passphrases, rejects a wrong passphrase, and exposes the cleartext child after success. Tests verify dismissal, a mounted or unlocked child blocking removal, disappearance, and the selected identity and capability recheck. Unpartitioned bytes measure gaps in the partition table, including alignment space. UDisks arbitrates whether hardware can be removed. [Drive methods](https://storaged.org/doc/udisks2-api/latest/gdbus-org.freedesktop.UDisks2.Drive.html) · [Encrypted volumes](https://storaged.org/doc/udisks2-api/latest/gdbus-org.freedesktop.UDisks2.Encrypted.html)
