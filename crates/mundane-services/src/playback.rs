//! Local video and audio playback. All GStreamer state changes and frame copies run off the UI thread.
mod pipeline;

use anyhow::Result;
use async_channel::Receiver;
use gstreamer as gst;
use pipeline::Pipeline;
pub use pipeline::{TrackInfo, inspect_audio};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Empty,
    Playing,
    Paused,
    Ended,
    Failed,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub phase: Phase,
    pub file: Option<PathBuf>,
    pub audio_request: Option<u64>,
    pub position: u64,
    pub duration: Option<u64>,
    pub error: Option<String>,
}
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
pub enum Command {
    Open(PathBuf),
    OpenAudio { path: PathBuf, request: u64 },
    Stop,
    Toggle,
    Seek(u64),
    Step(i64),
}
#[derive(Default)]
pub struct Update {
    pub status: Status,
    pub frame: Option<Frame>,
}
pub struct Playback {
    commands: mpsc::Sender<Command>,
    events: Receiver<()>,
    latest: Arc<Mutex<Update>>,
}

pub fn seek_position(fraction: f64, duration: u64) -> u64 {
    (fraction.clamp(0., 1.) * duration as f64).round() as u64
}
impl Playback {
    pub fn start() -> Self {
        Self::worker("pipewiresink")
    }
    #[cfg(feature = "test-audio")]
    pub fn silent_for_test() -> Self {
        Self::worker("fakesink")
    }
    fn worker(audio_sink: &'static str) -> Self {
        let (commands, rx) = mpsc::channel();
        let (signal, events) = async_channel::bounded(1);
        let latest = Arc::new(Mutex::new(Update::default()));
        let output = latest.clone();
        std::thread::spawn(move || {
            let mut player = Player::default();
            loop {
                let wait = if player.status.phase == Phase::Playing {
                    20
                } else {
                    100
                };
                match rx.recv_timeout(Duration::from_millis(wait)) {
                    Ok(command) => {
                        player.status.error = None;
                        if let Err(e) = player.command(command, audio_sink) {
                            player.status.error = Some(format!("{e:#}"));
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                let frame = match player.poll() {
                    Ok(frame) => frame,
                    Err(e) => {
                        player.media = None;
                        player.status.phase = Phase::Failed;
                        player.status.error = Some(e.to_string());
                        None
                    }
                };
                let mut update = output.lock().unwrap();
                if update.status != player.status || frame.is_some() {
                    if update.status.file != player.status.file {
                        update.frame = None;
                    }
                    update.status = player.status.clone();
                    if frame.is_some() {
                        update.frame = frame;
                    }
                    let _ = signal.try_send(());
                }
            }
        });
        Self {
            commands,
            events,
            latest,
        }
    }
    pub fn send(&self, cmd: Command) {
        let _ = self.commands.send(cmd);
    }
    pub fn events(&self) -> Receiver<()> {
        self.events.clone()
    }
    pub fn take(&self) -> Update {
        let mut latest = self.latest.lock().unwrap();
        Update {
            status: latest.status.clone(),
            frame: latest.frame.take(),
        }
    }
}

#[derive(Default)]
struct Player {
    media: Option<Pipeline>,
    status: Status,
}
impl Player {
    fn command(&mut self, cmd: Command, audio_sink: &str) -> Result<()> {
        if matches!(cmd, Command::Stop) {
            self.media = None;
            self.status = Status::default();
            return Ok(());
        }
        if let Command::Open(path) = cmd {
            let media = Pipeline::open(&path)?;
            media.play(true)?;
            self.status = Status {
                phase: Phase::Playing,
                file: Some(path),
                duration: media.duration(),
                ..Default::default()
            };
            self.media = Some(media);
            return Ok(());
        }
        if let Command::OpenAudio { path, request } = cmd {
            self.media = None;
            self.status = Status {
                file: Some(path.clone()),
                audio_request: Some(request),
                phase: Phase::Failed,
                ..Default::default()
            };
            let (media, _) = Pipeline::audio(&path, audio_sink)?;
            media.play(true)?;
            self.status.duration = media.duration();
            self.status.phase = Phase::Playing;
            self.media = Some(media);
            return Ok(());
        }
        let Some(media) = &self.media else {
            return Ok(());
        };
        match cmd {
            Command::Toggle => {
                let play = self.status.phase != Phase::Playing;
                if self.status.phase == Phase::Ended {
                    media.seek(0)?;
                }
                media.play(play)?;
                self.status.phase = if play { Phase::Playing } else { Phase::Paused };
            }
            Command::Seek(ms) => self.seek(ms)?,
            Command::Step(delta) => {
                let ms = media
                    .position()
                    .unwrap_or(self.status.position)
                    .saturating_add_signed(delta);
                self.seek(ms)?;
            }
            Command::Stop | Command::Open(_) | Command::OpenAudio { .. } => unreachable!(),
        }
        Ok(())
    }
    fn seek(&mut self, ms: u64) -> Result<()> {
        if let (Some(media), Some(duration)) = (&self.media, self.status.duration) {
            if self.status.phase == Phase::Ended {
                media.play(false)?;
                self.status.phase = Phase::Paused;
            }
            media.seek(ms.min(duration))?;
        }
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<Frame>> {
        let Some(media) = &self.media else {
            return Ok(None);
        };
        for msg in media.bus.iter() {
            match msg.view() {
                gst::MessageView::Error(e) => anyhow::bail!("{}", e.error()),
                gst::MessageView::Eos(_) => {
                    self.status.phase = Phase::Ended;
                    media.play(false)?;
                }
                _ => {}
            }
        }
        self.status.duration = media.duration().or(self.status.duration);
        if self.status.phase == Phase::Ended {
            self.status.position = self.status.duration.unwrap_or(self.status.position);
        } else if let Some(pos) = media.position() {
            self.status.position = pos;
        }
        media.frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_maps_to_clamped_seek_positions() {
        assert_eq!(seek_position(0.5, 11_501), 5751);
        assert_eq!(seek_position(-0.1, 12_000), 0);
        assert_eq!(seek_position(1.5, 12_000), 12_000);
        assert_eq!(seek_position(0.9, 0), 0);
    }
}
