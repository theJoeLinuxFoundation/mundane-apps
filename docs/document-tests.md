# Document Viewer

Document Viewer uses PDFium through the MIT/Apache-2.0 Rust binding `pdfium-render` 0.9.6. One open PDF supplies the rendered page, extracted Unicode text, and the page-space rectangles for each search hit. The viewer uses PDFium’s page-to-device transform to mark the matching text at the current zoom and display scale. Searches wrap across pages. Image-only scans display normally and require an existing text layer for search.

## Engine comparison

I rendered page two of the suite’s original survey fixture with PDFium 8149 and Hayro 0.9.0 at 840 × 560 pixels. Both preserved the chart outline, fills, grid lines, heading weight, body text and labels. Glyph edges differed slightly. This fixture exercises unembedded standard fonts, text and vector graphics. It does not measure complex-script shaping or scanned-image fidelity. Hayro also has a much larger upstream visual regression suite. [Hayro project and limitations](https://github.com/LaurenzV/hayro)

| On the same x86-64 host | PDFium | Hayro |
| --- | --- | --- |
| First page raster after opening the document | 1.25 ms | 1.55 ms |
| Median of 20 further rasters of that page | 0.40 ms | 0.29 ms |
| Text and search | Unicode page text, forward and backward search, and match rectangles through the binding | Interpreter glyphs and geometry are available to a custom device. The renderer does not provide a ready-made document search interface |
| Packaging | Build a shared C++ library using GN and Ninja, ship its notices, and update it separately | Build the Rust renderer and its dependencies through Cargo and distribute the selected fonts and CMaps with their notices |

The timing covers CPU rasterization with release builds, one open document and a reused Hayro render cache. File I/O, PNG encoding, GPUI, display upload and search were outside the timed region. Hayro was faster on repeated renders of this page. I chose PDFium because its text and coordinate APIs let the viewer search and highlight what it renders without maintaining a second text interpreter. [PDFium build documentation](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/README.md) · [PDFium public text API](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_text.h) · [Rust binding](https://docs.rs/pdfium-render/0.9.6/pdfium_render/)

## Packaging and licences

Install `libpdfium.so` on the dynamic loader’s system search path. The Rust build needs neither PDFium headers nor a PDFium binary at link time. Opening a PDF and running the tests require the library at runtime. Keep the binding’s explicit `pdfium_8125` API selection and test against the packaged engine when updating either. The tested engine was PDFium 159.0.8149.0, without V8 or XFA. The viewer has no PDF command-line helper.

PDFium’s licence file includes BSD-3-Clause and Apache-2.0 terms. Ship the component notices that correspond to joe’s build, including font, rasterizer and image-codec notices. FreeType offers its FreeType License. The tested x86-64 glibc library’s ELF dependencies were the system C, math and threading libraries, the loader and the approved GCC exception runtime. The suite ships no PDFium or Hayro binaries or fonts in this repository. [Upstream licence](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/LICENSE) · [Builds and third-party notices](https://github.com/bblanchon/pdfium-binaries) · [Workspace licence policy](plan.md#dependency-licences)

For the reference run I extracted the native library from the x86-64 manylinux wheel for pypdfium2 5.15.2. Python bindings are unnecessary to build or run the viewer. To reproduce that engine in a private test directory:

```sh
mkdir -p target/pdfium-test
python3 -m pip download --only-binary=:all: --no-deps --platform manylinux2014_x86_64 \
  --dest target/pdfium-test pypdfium2==5.15.2
wheel=$(find target/pdfium-test -maxdepth 1 -name 'pypdfium2-5.15.2-*.whl' -print -quit)
echo "dc31aea5b0fb9fefc9b537a65f4113980b2c40798445736a7e6565bc85161e37  $wheel" | sha256sum -c -
unzip -qo "$wheel" 'pypdfium2_raw/libpdfium.so' 'pypdfium2_raw/version.json' \
  '*/licenses/*' -d target/pdfium-test
export LD_LIBRARY_PATH="$PWD/target/pdfium-test/pypdfium2_raw${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
```

## Acceptance tests

The model checks page bounds, an unreadable file, case-insensitive matches across pages, the absence of matches and the device coordinates of a highlighted word. The GPUI headless test jumps to page two, finds text, changes zoom, checks focus, wraps a backward search and preserves the current page after a failed Open.

```sh
cargo fetch --locked
cargo test --workspace --offline --locked
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo build --workspace --release --offline --locked
export MUNDANE_DOCUMENT_VIEWER_BIN="$PWD/target/release/mundane-document-viewer"
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app document-viewer --compositor "$compositor" --profile "$profile" || exit 1
    done
done
tests/wayland/run --app document-viewer --at-spi
```

The six references show page two with “high water” highlighted at 125% document zoom. Display scales are 100%, 150% and 200%, with high contrast at 200%. The isolated headless Sway and labwc sessions use Pixman and Mesa lavapipe. The driver opens a second PDF through the private FileChooser portal. On the private accessibility bus, the inspector reads the actual sentence through AT-SPI Text, checks the focused query, page number and match count, and activates Zoom out, Done and Previous page. [Test dependencies](terminal-tests.md#software-rendered-wayland-tests)

The joe Linux VM test is pending on the packaged image. The three shared accessibility checks remain tracked in the [suite report](accessibility/report.md).
