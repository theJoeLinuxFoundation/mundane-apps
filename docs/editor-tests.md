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

The AT-SPI test reads the document and query while Find is focused and checks the shared buttons and status bar. [Current acceptance results](accessibility/report.md)

The joe Linux VM smoke test is pending until the distribution packages mundaneDE and Text Editor.

Round four adds saved indentation settings, reopen-closed-file and printing. Ctrl+Shift+I cycles tabs or 2, 4 and 8 spaces. A selected block can be indented and outdented as one undoable edit, preserving mixed LF and CRLF. Ctrl+Shift+T reopens a closed file from this window. Ctrl+Shift+P prints the current buffer and Ctrl+P keeps its existing Recent files shortcut.

Build with pkg-config, a C compiler and PangoCairo development files. The native helper links after its static archive to dynamic system Pango and Cairo, the same rendering libraries used by Fonts. It shapes and wraps UTF-8, paginates on A4 and sends an anonymous PDF descriptor to the desktop Print portal. The round-four Wayland check reads that descriptor through system PDFium, finds unsaved content on the first page and the final observation on the last, and verifies the original file on disk.
