use anyhow::{Context, Result, anyhow, bail, ensure};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use gstreamer_video::{VideoFrameRef, VideoInfo, prelude::*};
use std::{
    fs::File,
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use super::Frame;

fn factory(name: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(name)
        .build()
        .with_context(|| format!("GStreamer element {name} is unavailable"))
}

fn decoder(caps: &str) -> Result<&'static str> {
    match caps {
        "video/x-vp8" => Ok("vp8dec"),
        "video/x-vp9" => Ok("vp9dec"),
        "audio/x-opus" => Ok("opusdec"),
        "audio/x-vorbis" => Ok("vorbisdec"),
        _ => bail!(
            "Unsupported codec {caps}. Use WebM with VP8 or VP9 video and optional Opus or Vorbis audio."
        ),
    }
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
    sink: AppSink,
    _file: File,
}

impl Pipeline {
    pub fn open(path: &Path) -> Result<Self> {
        gst::init()?;
        let file = File::options()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open(path)?;
        ensure!(file.metadata()?.is_file(), "Choose a regular WebM file.");
        let pipeline = gst::Pipeline::new();
        let source = factory("fdsrc")?;
        source.set_property("fd", file.as_raw_fd());
        let demux = factory("matroskademux")?;
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
                let kind = caps
                    .structure(0)
                    .context("Empty stream caps")?
                    .name()
                    .as_str();
                let is_video = kind.starts_with("video/");
                if is_video {
                    if video.swap(true, Ordering::Relaxed) {
                        return Ok(());
                    }
                } else if !kind.starts_with("audio/") || audio.swap(true, Ordering::Relaxed) {
                    return Ok(());
                }
                let name = decoder(kind)?;
                let codec =
                    factory(name).with_context(|| format!("Missing decoder {name} for {kind}"))?;
                let mut chain = vec![factory("queue")?, codec];
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
            bail!("{}", e.error());
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
    pub fn seek(&self, ms: u64) -> Result<()> {
        self.pipeline.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::ClockTime::from_mseconds(ms),
        )?;
        Ok(())
    }
    pub fn frame(&self) -> Result<Option<Frame>> {
        let preroll = self.sink.try_pull_preroll(gst::ClockTime::ZERO);
        let Some(sample) = self.sink.try_pull_sample(gst::ClockTime::ZERO).or(preroll) else {
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
        assert!(
            decoder("video/x-h264")
                .unwrap_err()
                .to_string()
                .contains("video/x-h264")
        );
        gst::init().unwrap();
        assert!(
            factory("mundane-nonexistent-decoder")
                .unwrap_err()
                .to_string()
                .contains("mundane-nonexistent-decoder")
        );
    }
}
