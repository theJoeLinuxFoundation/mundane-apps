use anyhow::{Context, Result, anyhow, bail, ensure};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use gstreamer_video::{VideoFrameRef, VideoInfo, prelude::*};
use std::{
    fs::File,
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, OpenOptionsExt},
    },
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use super::Frame;

mod audio;
pub use audio::{TrackInfo, inspect_audio, waveform};

fn local_file(path: &Path) -> Result<File> {
    let file = File::options()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)?;
    ensure!(file.metadata()?.is_file(), "Choose a regular media file.");
    Ok(file)
}

fn factory(name: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(name)
        .build()
        .with_context(|| format!("GStreamer element {name} is unavailable"))
}

fn decoder(caps: &gst::CapsRef) -> Result<gst::Element> {
    let stream = caps.structure(0).context("Empty stream caps")?;
    let kind = stream.name().as_str();
    let name = match kind {
        "video/x-h264" => "openh264dec",
        "video/x-vp8" => "vp8dec",
        "video/x-vp9" => "vp9dec",
        "audio/x-opus" => "opusdec",
        "audio/x-vorbis" => "vorbisdec",
        "audio/mpeg" if matches!(stream.get::<i32>("mpegversion"), Ok(2 | 4)) => {
            let audio_decoder =
                gst::ElementFactoryType::DECODER | gst::ElementFactoryType::MEDIA_AUDIO;
            return gst::ElementFactory::factories_with_type(audio_decoder, gst::Rank::MARGINAL)
                .iter()
                .filter(|f| f.can_sink_all_caps(caps))
                .find_map(|f| f.create().build().ok())
                .context(
                    "No system GStreamer decoder for this AAC soundtrack. Install gst-libav.",
                );
        }
        _ => bail!("Unsupported codec {kind}."),
    };
    factory(name).with_context(|| format!("Missing decoder {name} for {kind}"))
}

fn attach(
    pipeline: &gst::Pipeline,
    pad: &gst::Pad,
    elements: &[gst::Element],
    tail: Option<&gst::Element>,
) -> Result<()> {
    pipeline.add_many(elements)?;
    gst::Element::link_many(elements)?;
    if let Some(tail) = tail {
        elements.last().unwrap().link(tail)?;
    }
    pad.link(
        &elements[0]
            .static_pad("sink")
            .context("Missing input pad")?,
    )?;
    for element in elements.iter().rev() {
        element.sync_state_with_parent()?;
    }
    Ok(())
}

pub(super) struct Pipeline {
    pipeline: gst::Pipeline,
    pub bus: gst::Bus,
    sink: Option<AppSink>,
    _file: File,
}

impl Pipeline {
    pub fn open(path: &Path) -> Result<Self> {
        gst::init()?;
        let file = local_file(path)?;
        let pipeline = gst::Pipeline::new();
        let mut header = [0; 8];
        let is_mp4 = file.read_exact_at(&mut header, 0).is_ok()
            && matches!(
                &header[4..],
                b"ftyp" | b"moov" | b"mdat" | b"free" | b"wide"
            );
        let (source, demux) = if is_mp4 {
            // A movie index can follow the media data. Allow qtdemux to pull
            // from the same opened inode even if the path is replaced.
            let source = factory("filesrc")?;
            source.set_property("location", format!("/proc/self/fd/{}", file.as_raw_fd()));
            (source, factory("qtdemux")?)
        } else {
            let source = factory("fdsrc")?;
            source.set_property("fd", file.as_raw_fd());
            (source, factory("matroskademux")?)
        };
        let converter = factory("videoconvertscale")?;
        let sink = factory("appsink")?.downcast::<AppSink>().unwrap();
        sink.set_caps(Some(
            &gst::Caps::builder("video/x-raw")
                .field("format", "BGRA")
                .field("width", gst::IntRange::new(1, 1920))
                .field("height", gst::IntRange::new(1, 1080))
                .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
                .build(),
        ));
        sink.set_max_buffers(1);
        sink.set_drop(true);
        sink.set_wait_on_eos(false);
        pipeline.add_many([&source, &demux, &converter, sink.upcast_ref()])?;
        source.link(&demux)?;
        converter.link(&sink)?;
        let weak = pipeline.downgrade();
        let seen_video = Arc::new(AtomicBool::new(false));
        let video = seen_video.clone();
        let audio = AtomicBool::new(false);
        demux.connect_pad_added(move |demux, pad| {
            let Some(pipeline) = weak.upgrade() else {
                return;
            };
            let result = (|| -> Result<()> {
                let caps = pad
                    .current_caps()
                    .context("Stream has no codec information")?;
                let stream = caps.structure(0).context("Empty stream caps")?;
                let kind = stream.name().as_str();
                let is_video = kind.starts_with("video/");
                if is_video {
                    if video.swap(true, Ordering::Relaxed) {
                        return Ok(());
                    }
                } else if !kind.starts_with("audio/") || audio.swap(true, Ordering::Relaxed) {
                    return Ok(());
                }
                let codec = decoder(&caps)?;
                let mut chain = vec![factory("queue")?];
                if kind == "video/x-h264" {
                    chain.push(factory("h264parse")?);
                }
                chain.push(codec);
                if is_video {
                    attach(&pipeline, pad, &chain, Some(&converter))
                } else {
                    let sink = factory("pipewiresink")?;
                    sink.set_property("client-name", "mundane-video");
                    chain.extend([factory("audioconvert")?, factory("audioresample")?, sink]);
                    attach(&pipeline, pad, &chain, None)
                }
            })();
            if let Err(e) = result {
                gst::element_error!(demux, gst::StreamError::Failed, ("{e:#}"));
            }
        });
        demux.connect_no_more_pads(move |demux| {
            if !seen_video.load(Ordering::Relaxed) {
                gst::element_error!(
                    demux,
                    gst::StreamError::TypeNotFound,
                    ("This file has no video track.")
                );
            }
        });
        Self::prepare(pipeline, file, Some(sink))
    }
    fn prepare(pipeline: gst::Pipeline, file: File, sink: Option<AppSink>) -> Result<Self> {
        let player = Self {
            bus: pipeline.bus().unwrap(),
            pipeline,
            sink,
            _file: file,
        };
        player.play(false)?;
        let msg = player
            .bus
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(10),
                &[gst::MessageType::Error, gst::MessageType::AsyncDone],
            )
            .context("Timed out preparing playback. Check the file and PipeWire output.")?;
        if let gst::MessageView::Error(e) = msg.view() {
            let error = e.error();
            if player.sink.is_some() && error.matches(gst::StreamError::Demux) {
                bail!("Could not read the video. The file may be damaged.");
            }
            bail!("{error}");
        }
        Ok(player)
    }
    pub fn play(&self, yes: bool) -> Result<()> {
        self.pipeline.set_state(if yes {
            gst::State::Playing
        } else {
            gst::State::Paused
        })?;
        Ok(())
    }
    pub fn position(&self) -> Option<u64> {
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|t| t.mseconds())
    }
    pub fn duration(&self) -> Option<u64> {
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|t| t.mseconds())
    }
    pub fn seek(&self, ms: u64, speed: super::Speed) -> Result<()> {
        self.pipeline.seek(
            speed.factor(),
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            gst::ClockTime::from_mseconds(ms),
            gst::SeekType::None,
            gst::ClockTime::NONE,
        )?;
        Ok(())
    }
    pub fn frame(&self) -> Result<Option<Frame>> {
        let Some(sink) = &self.sink else {
            return Ok(None);
        };
        let preroll = sink.try_pull_preroll(gst::ClockTime::ZERO);
        let Some(sample) = sink.try_pull_sample(gst::ClockTime::ZERO).or(preroll) else {
            return Ok(None);
        };
        let info = VideoInfo::from_caps(sample.caps().context("Video frame has no format")?)?;
        let frame = VideoFrameRef::from_buffer_ref_readable(
            sample.buffer().context("Video frame has no pixels")?,
            &info,
        )?;
        let stride = usize::try_from(frame.plane_stride()[0])?;
        let width = info.width() as usize * 4;
        ensure!(stride >= width && width != 0, "Invalid video stride");
        let mut pixels = Vec::with_capacity(width * info.height() as usize);
        for line in frame
            .plane_data(0)?
            .chunks(stride)
            .take(info.height() as usize)
        {
            pixels.extend_from_slice(
                line.get(..width)
                    .ok_or_else(|| anyhow!("Short video row"))?,
            );
        }
        ensure!(
            pixels.len() == width * info.height() as usize,
            "Incomplete video frame"
        );
        Ok(Some(Frame {
            width: info.width(),
            height: info.height(),
            pixels,
        }))
    }
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_codec_names_the_stream() {
        gst::init().unwrap();
        let av1 = gst::Caps::builder("video/x-av1").build();
        assert!(
            decoder(&av1)
                .unwrap_err()
                .to_string()
                .contains("video/x-av1")
        );
        assert!(
            factory("mundane-nonexistent-decoder")
                .unwrap_err()
                .to_string()
                .contains("mundane-nonexistent-decoder")
        );
    }
}
