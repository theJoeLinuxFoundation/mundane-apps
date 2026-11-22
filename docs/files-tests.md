# Testing Files

Run the workspace tests as a regular desktop user. The service tests operate on temporary directories. They cover collisions with existing names and dangling symlinks, invalid renames, directory copies, a copy into a folder with write permission removed, and thumbnail-cache reuse, corruption and invalidation.

```sh
cargo test --workspace --locked
```

The GPUI test opens a folder by keyboard, renames a file, copies it and pastes it in the same folder. It checks focus on Cancel, presses Escape, checks the listing has focus again, and verifies the original file is intact. A second paste chooses Keep both and checks the copy's contents.

The service tests cover reviewed copy and move replacements, stale metadata, symbolic links, whole folders and an attempted replacement of an ancestor. A folder’s displayed size is its directory entry size in bytes. Replacing a folder replaces its contents as a whole.

## Wayland acceptance

Build Files once, then run the six compositor and scale combinations. Set `MUNDANE_FILES_BIN` to the release binary’s absolute path when testing a release build.

```sh
cargo build --locked -p mundane-files
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app files --compositor "$compositor" --profile "$profile" || exit 1
    done
done
```

Each run creates a private home, session bus, clipboard and Wayland socket. Sway and labwc use the headless wlroots backend with Pixman. Files uses Mesa lavapipe. Sway negotiates server decorations for its tiled window and labwc negotiates client decorations. The profiles capture the same 720 × 440 logical window at 100%, 150%, and 200% with high contrast. Each capture must match the reviewed PNG pixel for pixel. The dialog initially focuses Cancel and compares two different files named `notes.txt`. Each row gives the item’s type, size in bytes and modification date in UTC. [Compositor, font and capture dependencies](terminal-tests.md#software-rendered-wayland-tests)

The runner cancels a replacement, checks the old contents, confirms Replace by keyboard and checks the new contents. It also verifies Keep both, a cut and paste into another folder, the file received by a private default MIME handler, and a confirmed move to the private Trash with its recovery metadata. The Sway runs exercise GIO opening a file when the portal is unavailable. The labwc runs use a private OpenURI test service that reads the passed file descriptor, cancels one request and accepts the next. Only the accepted request launches the handler.

Install GNU coreutils with `mv --update=none-fail` support, GLib's `gio` and `gsettings`, desktop interface GSettings schemas and the Python GObject bindings for GIO. The test creates its own MIME association, icon theme and thumbnail source. For inspecting a changed image, `MUNDANE_CAPTURE_ONLY=1` writes the new capture to `target/wayland/`. Run again with the variable unset to verify the reviewed reference.

The round-three test also copies a selection into a folder by keyboard, drags both files into it to move them, and drops a selection into a second Files process. It checks that both destination files match the originals and that copying between windows preserves the sources. [Comparison screenshots](parity.md#round-three)

## Accessibility probe

Run `tests/wayland/run --app files --at-spi` to inspect the live location dialog, field and actions alongside the GTK control. The conflict probe also checks the dialog’s accessible comparison and its three named buttons. [Current acceptance results](accessibility/report.md)
