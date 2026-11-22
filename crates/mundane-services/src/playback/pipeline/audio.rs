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
        sink.set_property("sync", true);
        pipeline.add_many([&source, &convert, &resample, &sink])?;
        gst::Element::link_many([&convert, &resample, &sink])?;
        if &magic[..4] == b"fLaC" {
            let parse = factory("flacparse")?;
            let codec = factory("flacdec")?;
            pipeline.add_many([&parse, &codec])?;
            gst::Element::link_many([&source, &parse, &codec, &convert])?;
        } else if &magic[..4] == b"RIFF" && &magic[8..] == b"WAVE" {
            let parse = factory("wavparse")?;
            pipeline.add(&parse)?;
            gst::Element::link_many([&source, &parse, &convert])?;
        } else if &magic[..4] == b"OggS" {
            let demux = factory("oggdemux")?;
            pipeline.add(&demux)?;
            source.link(&demux)?;
            let weak = pipeline.downgrade();
            let found = Arc::new(AtomicBool::new(false));
            let linked = found.clone();
            demux.connect_pad_added(move |demux, pad| {
                let Some(p) = weak.upgrade() else { return };
                let result = (|| -> Result<()> {
                    let caps = pad.current_caps().context("Missing Ogg stream caps")?;
                    let kind = caps.structure(0).context("Empty Ogg caps")?.name().as_str();
                    let name = match kind {
                        "audio/x-opus" => "opusdec",
                        "audio/x-vorbis" => "vorbisdec",
                        _ => return Ok(()),
                    };
                    if linked.swap(true, Ordering::Relaxed) {
                        return Ok(());
                    }
                    attach(
                        &p,
                        pad,
                        &[factory("queue")?, factory(name)?],
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
                        ("No Opus or Vorbis track in this Ogg file.")
                    );
                }
            });
        } else {
            bail!("Unsupported audio file. Choose FLAC, PCM WAV, or Ogg Opus or Vorbis.")
        }
        Ok((Self::prepare(pipeline, file, None)?, sink))
    }
}
