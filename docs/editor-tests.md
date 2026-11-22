# Testing Text Editor

Run `cargo test --workspace --locked` as a regular desktop user. The document tests round-trip UTF-8 with a BOM, a combining accent, CJK, emoji, mixed CRLF and LF, and a symbolic link. They check that a failed write preserves the original bytes, that external changes survive a stale save, and that invalid UTF-8 is rejected.

The GPUI tests edit a multiline document, find the new text, dismiss a compositor close request and save. They check focus and file contents. They also cancel and retry the first save of an untitled document and exercise a failed save from the close prompt. That failure leaves the edited buffer available for Save As.

## Wayland acceptance

Build the editor once and run each command in a regular user session. The harness creates a private home directory, session bus, clipboard and headless Wayland socket. Sway and labwc use Pixman. The editor uses Vulkan through Mesa lavapipe.

```sh
cargo build --release --locked -p mundane-text-editor
export MUNDANE_TEXT_EDITOR_BIN="$PWD/target/release/mundane-text-editor"
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app text-editor --compositor "$compositor" --profile "$profile" || exit 1
    done
done
tests/wayland/run --app text-editor --at-spi
```

Each run opens a CRLF document by path, appends text and finds two words in the edit. The six reviewed references show the selected match and unsaved indicator at 100%, 150%, and 200% with high contrast and increased document text size. Sway negotiates server decorations and labwc negotiates client decorations. Each run copies the match to the private clipboard, cancels a compositor close request, saves, and compares the file bytes. The labwc run also closes through the shared header while the document is dirty.

A private FileChooser service exercises Save As, Open cancellation and opening another document over the session bus. The test compares the saved copy byte for byte and checks the title after opening the second file. Install the Python GObject bindings for GIO along with the compositor and rendering dependencies. [Test dependencies](terminal-tests.md#software-rendered-wayland-tests)

The private AT-SPI probe ran with Find focused. The inspector and Orca both identified the GTK positive control. Neither listed Text Editor. The editor's text fields, buttons and confirmation dialog have role and name declarations in the shared UI, ready for the GPUI bridge work.

The joe Linux VM smoke test is pending until the distribution packages mundaneDE and Text Editor.
