# Music playback and packaging

Music plays a queue of local FLAC, PCM WAV, Ogg Opus and Ogg Vorbis files. The desktop entry accepts multiple paths. Add tracks appends the local files chosen in the FileChooser portal. Each queue entry has its own identity, including repeated copies of the same file. Reordering a playing entry preserves its playback position. End of stream advances in the current queue order. Playback stops at the last entry and offers Replay.

Title, artist, album and duration come from a GStreamer preroll on a worker thread. The metadata probe uses a silent sink and falls back to the filename when the title is absent. Audio decoding and PipeWire output run on the shared playback worker. An open request identifies each playback session so that a late notification from a previous track cannot advance the new one.

## What joe must ship

Music shares Video’s GStreamer 1.22+ runtime and its dynamically linked core, app and video libraries. Install PipeWire with a session manager, the desktop FileChooser portal, and these GStreamer plugin modules with their runtime dependencies. The application selects the parsers, decoders and sink explicitly.

| Supplier | Modules | Purpose |
| --- | --- | --- |
| GStreamer core | `coreelements` | Seekable file source, queue and silent metadata sink |
| Base | `ogg`, `opus`, `vorbis` | Ogg demuxer, libopus and libvorbis decoders |
| Base | `audioconvert`, `audioresample` | Negotiate the output format and sample rate |
| Good | `audioparsers`, `flac`, `wavparse` | FLAC parser and libFLAC decoder, PCM WAV parser |
| Base and Good | `app`, `audiofx` | Decode 8 kHz mono samples into 192 peak bins in a separate pipeline. Preserve pitch with `scaletempo` during playback. |
| Good and system gst-libav | `mpg123`, `isomp4`, `libav` | Decode MP3 including ID3-tagged files and AAC in M4A. |
| PipeWire | `pipewire` | Output to the session’s default audio sink |

The waveform reader never opens an audio device. Changing tracks or closing the window cancels analysis. Speed changes seek from the current media position, retain the paused state and carry over to the next track. The slider accepts the same pointer, keyboard and accessibility actions while the waveform is loading. [Shared media dependencies](video-tests.md)

## Acceptance tests

The queue test covers duplicate paths, reordering, the successor at end of stream, and the filename fallback. The GPUI headless test decodes the four fixtures against a clocked silent sink. It reads the tags, reorders the queue by keyboard, skips, pauses, advances at actual end of stream and replays the final track.

The Wayland test runs the release binary against private PipeWire, WirePlumber, the FileChooser portal and a headless compositor. It appends an untagged WAV, reorders Opus ahead of Vorbis, and records 330, 440, 550 and 660 Hz respectively from the private sink across all four formats. It checks the loaded plugin mappings against the shipping list. Six pixel-exact references show Harbour Lights paused at five seconds in headless Sway and labwc at 100%, 150%, and 200% with high contrast and larger type. GPUI renders with Mesa lavapipe. The private AT-SPI test verifies selected-track identity, metadata, seek position and reordering through accessible button actions. [Original audio fixture generator](../tests/fixtures/music/generate.py)

```sh
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build --release --locked --offline -p mundane-music
export MUNDANE_MUSIC_BIN="$PWD/target/release/mundane-music"
for compositor in sway labwc; do
    for profile in standard fractional contrast; do
        tests/wayland/run --app music --compositor "$compositor" --profile "$profile" || exit 1
    done
done
tests/wayland/run --app music --at-spi
tests/wayland/run --app music --at-spi --profile contrast
```

Set `MUNDANE_TEST_GST_PLUGIN_DIRS` if the distro splits these plugins across prefixes. The packaged joe Linux VM check remains pending on the image.
