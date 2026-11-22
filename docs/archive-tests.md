# Archive Manager

The reader links dynamically to the system BSD-licensed libarchive (tested with 3.8.10). Ship libarchive with built-in ZIP/deflate, tar and gzip support. Its gzip registration must succeed without an external filter. The reader takes entry names from libarchive as filesystem bytes, so display conversion does not change which entry is extracted.

Extraction creates a new folder named after the archive inside the chosen destination. The service streams files into a private, mode-0700 staging directory. It rejects absolute paths, parent components in entry names, escaping link targets and special files. It writes regular files before creating links, resolves link targets inside the staged selection, and requires a selected regular-file target for a hard link. Incomplete selections with unresolved links fail cleanly. It restores file contents and directory structure with user-owned permissions. A no-replace rename publishes the completed directory. An existing output folder, including a symlink with that name, causes the operation to fail.

Service tests cover tar hard links and relative symbolic links, invalid member names, escaping symbolic and hard links, omitted link targets, cleanup and output collisions. The GPUI test marks two nested ZIP members, chooses a directory through the test portal and checks the exact resulting files. The six compositor references show the selected tides.csv entry. The private AT-SPI test checks the selection, its marked state and the extraction action names.

```sh
cargo test --workspace --locked --offline
cargo build --release --locked --offline -p mundane-archive-manager
export MUNDANE_ARCHIVE_MANAGER_BIN="$PWD/target/release/mundane-archive-manager"
for c in sway labwc; do
    for p in standard fractional contrast; do
        tests/wayland/run --app archive-manager --compositor "$c" --profile "$p" || exit 1
    done
done
tests/wayland/run --app archive-manager --at-spi
```

Sway and labwc use headless outputs and Pixman. GPUI renders through Mesa lavapipe at scales 1, 1.5 and 2, with larger type and high contrast at scale 2. The joe image smoke test remains pending.

Round four creates a tar.xz from selected files, adds a second file and confirms removal through the keyboard. Python’s tarfile independently reads the result. Native tests round-trip ZIP, tar, gzip and xz, including a UTF-8 filename, duplicate-name conflicts and stale reviewed files. The writer streams retained libarchive entries into a temporary neighbour. A successful close, sync and version check precede publication. New archives use no-replace creation.
