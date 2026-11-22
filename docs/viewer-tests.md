# Testing Image Viewer

The original BSD-3-Clause fixture “North Point.jpg” stores a 300 × 480 JPEG with Exif orientation 6. The decoder test checks the upright dimensions of 480 × 300, the sun at upper left and the dark foreground at lower right. A second test checks the exact RGBA value of a translucent PNG. Removing the final JPEG marker or truncating the final PNG chunk must produce an error. Decoding runs off the UI thread. The viewer accepts regular files up to 64 MiB, with at most 8,192 pixels per side and 33,554,432 pixels total.

The GPUI headless test opens the fixture, reaches Open with F6, returns focus to the picture, zooms and pans, then tabs to Fit and presses Enter. It checks the restored scale and position and verifies that a subsequent failed Open keeps the current picture.

```sh
cargo test --workspace --locked
cargo build --workspace --release --locked
export MUNDANE_IMAGE_VIEWER_BIN="$PWD/target/release/mundane-image-viewer"
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app image-viewer --compositor "$compositor" --profile "$profile" || exit 1
    done
done
tests/wayland/run --app image-viewer --at-spi
```

Sway and labwc run on isolated headless outputs with Pixman. GPUI uses Mesa lavapipe. Each run opens the oriented JPEG by path, resizes the window, verifies that zooming and panning change the canvas pixels, and returns to fit. Six references compare every RGBA pixel at 100%, 150% and 200%, with high contrast at 200%. The runner then cancels one native file chooser request and opens a PNG through the private FileChooser portal. [Wayland test dependencies](terminal-tests.md#software-rendered-wayland-tests)

On the private accessibility bus, Orca lists Image Viewer as an application. The AT-SPI inspector checks the image role, filename, focus, upright dimensions and the four named buttons. It invokes Zoom in, Zoom out and Fit through their Click actions and checks the resulting zoom descriptions. [Suite accessibility report and three open checks](accessibility/report.md)

The desktop entry registers `image/png` and `image/jpeg` with a single local path argument. The packaged joe Linux VM smoke test is pending on the image.
