# Disk Usage

Disk Usage opens a local directory through the portal or from a path argument. It scans as the desktop user on a background worker. Allocated space uses the Linux block count in 512-byte units, including directory blocks. File sizes uses logical byte lengths and excludes directory metadata. Each directory lists its immediate children in descending size order. The bars show each child's share of the displayed total.

The traversal pins open directories, skips other filesystems and treats symlinks as leaves. It counts an inode once across the scan. For hard links, the first entry in a sorted depth-first traversal receives the bytes. Other links remain visible. An unreadable entry carries its error and makes the containing total a lower bound. Cancelling a rescan retains the last complete result and prevents the cancelled worker from publishing late results. Drill-down uses the scanned tree.

The service test creates hard links, a symlink and an unreadable directory. The headless GPUI test opens a child, navigates back and cancels a rescan. Six exact references show folder and file sizes with the unreadable directory selected, using headless Sway and labwc with Pixman and Mesa lavapipe. The fixture uses logical sizes so the references are independent of filesystem allocation. Run the Wayland test as a regular user to exercise permission denial. The private AT-SPI inspector checks the selected entry, its error, a child size and the incomplete total.

```sh
cargo test --workspace --locked --offline
cargo build --release --locked --offline -p mundane-disk-usage
export MUNDANE_DISK_USAGE_BIN="$PWD/target/release/mundane-disk-usage"
for c in sway labwc; do
    for p in standard fractional contrast; do
        tests/wayland/run --app disk-usage --compositor "$c" --profile "$p" || exit 1
    done
done
tests/wayland/run --app disk-usage --at-spi
```

Install the binary and its desktop entry alongside the suite’s Wayland and FileChooser dependencies. The scanner uses Linux filesystem APIs and procfs. The packaged joe image check remains pending.
