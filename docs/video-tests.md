# Video playback and packaging

Video opens a regular local file through the FileChooser portal or the `video/webm` desktop entry. It plays the first video track and the first audio track, if present. Pause holds the picture, seeking preserves the current play/pause state, and Replay starts at zero after end of stream. A failed Open reports the error and preserves the current clip.

## What joe must ship

Build against GStreamer 1.22 or newer, the Base development libraries for app and video, and their pkg-config files. The Rust bindings are MIT/Apache-2.0. The release ELF has dynamic dependencies on `libgstreamer-1.0.so.0`, `libgstapp-1.0.so.0`, `libgstvideo-1.0.so.0`, GLib and GObject. Install the corresponding runtime libraries and the following plugin modules with their native dependencies. The reference run used GStreamer 1.28.8.

| Supplier | GStreamer plugin | Required elements and purpose |
| --- | --- | --- |
| GStreamer core | `coreelements` | `fdsrc` reads the open file and `queue` decouples the tracks |
| Good | `matroska` | `matroskademux` reads the WebM container |
| Good | `vpx` | `vp8dec` and `vp9dec` use libvpx |
| Base | `opus`, `vorbis` | `opusdec` and `vorbisdec` use libopus and libvorbis |
| Base | `videoconvertscale`, `app` | `videoconvertscale` produces square-pixel BGRA frames, and `appsink` hands them to GPUI |
| Base | `audioconvert`, `audioresample` | Negotiate sample format, channels and rate with the output |
| PipeWire | `pipewire` | `pipewiresink` sends audio directly to the session’s PipeWire server and default output |

The pipeline constructs these elements by name. There is no codec autoplugging or external media-player process. Core, Base and Good plugins in this list report LGPL; the PipeWire plugin reports MIT/X11. libvpx, libopus and libvorbis are BSD-licensed. The test makes only these ten plugin modules visible, plays both codec combinations, then checks the plugin mappings in the running release process. The shipping list excludes Ugly, x264 and gst-libav. [GStreamer licensing](https://gstreamer.freedesktop.org/documentation/plugin-development/appendix/licensing-advisory.html) · [VP8 decoder](https://gstreamer.freedesktop.org/documentation/vpx/vp8dec.html) · [Matroska demuxer](https://gstreamer.freedesktop.org/documentation/matroska/matroskademux.html)

Pipeline state changes, bus errors and frame reads run on a worker thread. Decoded BGRA is limited to 1920 × 1080, row padding is removed, and the UI receives the most recent frame. The clock, play/pause/replay button, step buttons and seek slider are in mundane-ui for reuse by Music. The slider exposes its range, position and value setter through AccessKit.

## Acceptance tests

```sh
cargo fetch --locked
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build --workspace --release --locked --offline
export MUNDANE_VIDEO_BIN="$PWD/target/release/mundane-video"
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app video --compositor "$compositor" --profile "$profile" || exit 1
    done
done
tests/wayland/run --app video --at-spi
tests/wayland/run --missing-video-decoder
```

The headless GPUI test opens a real silent clip, pauses, seeks to five seconds, runs through end-of-stream and replays. It checks focus, dimensions, duration and recovery after opening an invalid file. Service tests check fractional seek mapping, clamping, unsupported codecs and missing elements. In a separate private session the VP8 plugin is removed from the registry. The AT-SPI inspector verifies that the running app names the missing `vp8dec` decoder and `video/x-vp8` stream.

The Wayland tests require the listed plugins, PipeWire, WirePlumber, `wpctl` and `pw-record` in addition to the usual compositor dependencies. Set `MUNDANE_TEST_GST_PLUGIN_DIRS` to a colon-separated list when plugins are split across prefixes. Each run creates its own PipeWire server, null sink, registry and D-Bus session. WirePlumber runs its policy-only profile. A monitor records the fixture’s 440 Hz tone at 48 kHz from the private sink for **both** VP8/Opus and VP9/Vorbis. The VP9 clip opens through the private FileChooser portal. [Compositor dependencies](terminal-tests.md#software-rendered-wayland-tests)

Six pixel-exact references show the sailing clip paused at 0:05 of 0:12. Sway and labwc use headless outputs with Pixman; GPUI uses Mesa lavapipe. Output scales are 100%, 150% and 200%, with high contrast at 200%. The private AT-SPI test reads the slider’s elapsed time and duration, sets the time to seven seconds, steps back five seconds and toggles playback. The original WebM fixtures and their generator use the repository’s BSD-3-Clause licence. [Fixture generator](../tests/fixtures/coastal_passage.py)

The packaged joe Linux VM smoke test is pending on the image.
