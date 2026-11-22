//! A shell session backed by Alacritty's Linux PTY, parser and I/O thread.
use std::{io, sync::Arc};

use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    event_loop::{EventLoop, EventLoopSender, Msg},
    grid::Dimensions,
    sync::FairMutex,
    term::{Config, Term},
    tty::{self, Options},
};
use async_channel::{Receiver, Sender};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
}

impl Dimensions for GridSize {
    fn columns(&self) -> usize {
        self.cols.into()
    }
    fn screen_lines(&self) -> usize {
        self.rows.into()
    }
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }
}

#[derive(Clone)]
pub struct Events(Sender<Event>);
impl EventListener for Events {
    fn send_event(&self, event: Event) {
        if matches!(
            event,
            Event::Wakeup
                | Event::Title(_)
                | Event::ResetTitle
                | Event::Exit
                | Event::PtyWrite(_)
                | Event::ColorRequest(..)
                | Event::TextAreaSizeRequest(_)
        ) {
            let _ = self.0.try_send(event);
        }
    }
}

pub struct Session {
    pub screen: Arc<FairMutex<Term<Events>>>,
    channel: EventLoopSender,
    size: WindowSize,
    pid: u32,
    master: std::fs::File,
}

impl Session {
    pub fn open(mut options: Options, size: WindowSize) -> io::Result<(Self, Receiver<Event>)> {
        options.env.insert("TERM".into(), "xterm-256color".into());
        options.env.insert("COLORTERM".into(), "truecolor".into());
        let dimensions = GridSize {
            cols: size.num_cols,
            rows: size.num_lines,
        };
        let (tx, rx) = async_channel::unbounded();
        let events = Events(tx);
        let screen = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &dimensions,
            events.clone(),
        )));
        let pty = tty::new(&options, size, 0)?;
        let pid = pty.child().id();
        let master = pty.file().try_clone()?;
        let io = EventLoop::new(screen.clone(), events, pty, true, false)?;
        let channel = io.channel();
        io.spawn();
        Ok((
            Self {
                screen,
                channel,
                pid,
                master,
                size,
            },
            rx,
        ))
    }

    /// The foreground job can have a different process group; a shell waiting at
    /// its prompt may still own background or stopped children.
    pub fn running_job(&self) -> Option<String> {
        let fg = rustix::termios::tcgetpgrp(&self.master)
            .ok()
            .map(|p| p.as_raw_nonzero().get() as u32);
        if let Some(pid) = fg.filter(|&p| p != self.pid) {
            return Some(
                std::fs::read_to_string(format!("/proc/{pid}/comm"))
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_else(|_| "Foreground job".into()),
            );
        }
        let children =
            match std::fs::read_to_string(format!("/proc/{0}/task/{0}/children", self.pid)) {
                Ok(s) => s,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
                Err(_) => return Some("Job status unavailable".into()),
            };
        for pid in children.split_whitespace() {
            if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat"))
                && let Some((name, tail)) = stat.rsplit_once(") ")
                && !tail.starts_with("Z ")
            {
                return Some(
                    name.split_once('(')
                        .map_or(name, |(_, comm)| comm)
                        .to_string(),
                );
            }
        }
        None
    }

    pub fn working_directory(&self) -> Option<std::path::PathBuf> {
        std::fs::read_link(format!("/proc/{}/cwd", self.pid))
            .ok()
            .filter(|p| p.is_dir())
    }

    pub fn size(&self) -> WindowSize {
        self.size
    }

    pub fn resize(&mut self, size: WindowSize) {
        if (
            size.num_cols,
            size.num_lines,
            size.cell_width,
            size.cell_height,
        ) == (
            self.size.num_cols,
            self.size.num_lines,
            self.size.cell_width,
            self.size.cell_height,
        ) {
            return;
        }
        self.screen.lock().resize(GridSize {
            cols: size.num_cols,
            rows: size.num_lines,
        });
        self.size = size;
        let _ = self.channel.send(Msg::Resize(size));
    }

    pub fn write(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.channel.send(Msg::Input(bytes.into()));
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.channel.send(Msg::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::{
        index::{Column, Line, Point},
        tty::Shell,
    };
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    #[test]
    fn child_observes_resize_and_is_reaped() {
        let options = Options {
            shell: Some(Shell::new(
                "/bin/sh".into(),
                vec![
                    "-c".into(),
                    "read answer; printf 'pid=%s size=' \"$$\"; stty size; read done".into(),
                ],
            )),
            ..Options::default()
        };
        let size = WindowSize {
            num_lines: 20,
            num_cols: 80,
            cell_width: 9,
            cell_height: 20,
        };
        let (mut session, events) = Session::open(options, size).unwrap();
        session.resize(WindowSize {
            num_lines: 32,
            num_cols: 91,
            ..size
        });
        session.write(b"go\n".to_vec());
        let deadline = Instant::now() + Duration::from_secs(10);
        let pid = loop {
            let text = session.screen.lock().bounds_to_string(
                Point::new(Line(0), Column(0)),
                Point::new(Line(5), Column(90)),
            );
            if let Some((_, tail)) = text.split_once("pid=")
                && let Some((pid, _)) = tail.split_once(" size=32 91")
            {
                break pid.to_owned();
            }
            assert!(
                Instant::now() < deadline,
                "child did not report its resized PTY: {text:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        session.write(b"done\n".to_vec());
        while !matches!(events.try_recv(), Ok(Event::Exit)) {
            assert!(Instant::now() < deadline, "missing child exit event");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }
}
