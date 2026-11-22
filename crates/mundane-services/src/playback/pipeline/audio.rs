use super::*;
use std::os::unix::fs::FileExt;

/// Queue labels and duration, read at preroll without opening an audio device.
#[derive(Clone, Debug, Default)]
pub struct TrackInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<u64>,
}
impl TrackInfo {
    pub fn display_title(&self, path: &Path) -> String {
        self.title.clone().unwrap_or_else(|| {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
    }
}
pub fn inspect_audio(path: &Path) -> Result<TrackInfo> {
    let (media, sink) = Pipeline::audio(path, "fakesink")?;
    let mut info = TrackInfo {
        duration: media.duration(),
        ..Default::default()
    };
    let pad = sink.static_pad("sink").context("Missing audio sink pad")?;
    for idx in 0.. {
        let Some(event) = pad.sticky_event::<gst::event::Tag>(idx) else {
            break;
        };
        let tags = event.tag();
        for (field, value) in [
            (&mut info.title, tags.get::<gst::tags::Title>()),
            (&mut info.artist, tags.get::<gst::tags::Artist>()),
            (&mut info.album, tags.get::<gst::tags::Album>()),
        ] {
            if let Some(value) = value
                .map(|v| v.get().trim().to_owned())
                .filter(|v| !v.is_empty())
            {
                *field = Some(value)
            }
        }
    }
    Ok(info)
}

impl Pipeline {
    pub(in crate::playback) fn audio(path: &Path, output: &str) -> Result<(Self, gst::Element)> {
        gst::init()?;
        let file = local_file(path)?;
        let mut magic = [0; 12];
        file.read_exact_at(&mut magic, 0)
            .context("Incomplete audio file")?;
        let pipeline = gst::Pipeline::new();
        // filesrc supports pull scheduling for indexed Ogg seeks and duration.
        // Hold the validated inode across probing and playback.
        let source = factory("filesrc")?;
        source.set_property("location", format!("/proc/self/fd/{}", file.as_raw_fd()));
        let convert = factory("audioconvert")?;
        let resample = factory("audioresample")?;
        let sink = factory(output)?;
        if output == "pipewiresink" {
            sink.set_property("client-name", "mundane-music");
        }
        sink.set_property("sync", output != "appsink");
        if let Some(analysis) = sink.downcast_ref::<AppSink>() {
            analysis.set_caps(Some(
                &gst::Caps::builder("audio/x-raw")
                    .field("format", "S16LE")
                    .field("channels", 1i32)
                    .field("rate", 8000i32)
                    .build(),
            ));
            analysis.set_max_buffers(4);
        }
        pipeline.add_many([&source, &convert, &resample, &sink])?;
        if output == "pipewiresink" {
            let tempo = factory("scaletempo")?;
            pipeline.add(&tempo)?;
            gst::Element::link_many([&convert, &resample, &tempo, &sink])?;
        } else {
            gst::Element::link_many([&convert, &resample, &sink])?;
        }
        if &magic[..4] == b"fLaC" {
            let parse = factory("flacparse")?;
            let codec = factory("flacdec")?;
            pipeline.add_many([&parse, &codec])?;
            gst::Element::link_many([&source, &parse, &codec, &convert])?;
        } else if &magic[..4] == b"RIFF" && &magic[8..] == b"WAVE" {
            let parse = factory("wavparse")?;
            pipeline.add(&parse)?;
            gst::Element::link_many([&source, &parse, &convert])?;
        } else if magic.starts_with(b"ID3") || (magic[0] == 0xff && magic[1] & 0xe6 == 0xe2) {
            let parser = factory("mpegaudioparse")?;
            let codec = factory("mpg123audiodec")?;
            pipeline.add_many([&parser, &codec])?;
            gst::Element::link_many([&source, &parser, &codec, &convert])?;
        } else if &magic[..4] == b"OggS" || &magic[4..8] == b"ftyp" {
            let demux = factory(if &magic[..4] == b"OggS" {
                "oggdemux"
            } else {
                "qtdemux"
            })?;
            pipeline.add(&demux)?;
            source.link(&demux)?;
            let weak = pipeline.downgrade();
            let found = Arc::new(AtomicBool::new(false));
            let linked = found.clone();
            demux.connect_pad_added(move |demux, pad| {
                let Some(p) = weak.upgrade() else { return };
                let result = (|| -> Result<()> {
                    let caps = pad.current_caps().context("Missing audio track caps")?;
                    let kind = caps
                        .structure(0)
                        .context("Empty audio caps")?
                        .name()
                        .as_str();
                    if !kind.starts_with("audio/") {
                        return Ok(());
                    }
                    if linked.swap(true, Ordering::Relaxed) {
                        return Ok(());
                    }
                    attach(
                        &p,
                        pad,
                        &[factory("queue")?, decoder(&caps)?],
                        Some(&convert),
                    )
                })();
                if let Err(e) = result {
                    gst::element_error!(demux, gst::StreamError::Failed, ("{e:#}"));
                }
            });
            demux.connect_no_more_pads(move |demux| {
                if !found.load(Ordering::Relaxed) {
                    gst::element_error!(
                        demux,
                        gst::StreamError::TypeNotFound,
                        ("No audio track in this file.")
                    );
                }
            });
        } else {
            bail!("Unsupported audio file. Choose MP3, M4A, FLAC, PCM WAV, or Ogg Opus or Vorbis.")
        }
        Ok((Self::prepare(pipeline, file, None)?, sink))
    }
}

/// Peak amplitudes at evenly spaced media times, decoded without opening an audio device.
pub fn waveform(path: &Path, cancelled: &AtomicBool) -> Result<Vec<f32>> {
    const BINS: usize = 192;
    let (media, sink) = Pipeline::audio(path, "appsink")?;
    let sink = sink.downcast::<AppSink>().expect("created appsink");
    let duration = media
        .duration()
        .filter(|ms| *ms > 0)
        .context("No track duration")?;
    let mut peaks = vec![0f32; BINS];
    let mut offset = 0;
    media.play(true)?;
    while !cancelled.load(Ordering::Relaxed) {
        if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(150)) {
            let buffer = sample.buffer().context("Missing PCM samples")?;
            let data = buffer.map_readable()?;
            if let Some(pts) = buffer.pts() {
                offset = pts.mseconds().saturating_mul(8);
            }
            for bytes in data.as_slice().as_chunks::<2>().0 {
                let bin = (offset.saturating_mul(BINS as u64) / duration.saturating_mul(8))
                    .min(BINS as u64 - 1);
                let amplitude = i16::from_le_bytes(*bytes).unsigned_abs() as f32 / 32768.;
                peaks[bin as usize] = peaks[bin as usize].max(amplitude);
                offset += 1;
            }
        } else if let Some(msg) = media.bus.pop_filtered(&[gst::MessageType::Error]) {
            if let gst::MessageView::Error(error) = msg.view() {
                bail!("{}", error.error());
            }
        } else if sink.is_eos() {
            return Ok(peaks);
        }
    }
    bail!("Waveform cancelled")
}
