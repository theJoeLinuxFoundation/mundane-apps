mod queue;
#[cfg(test)]
mod tests;
mod view;
use gpui::{prelude::*, *};
use mundane_services::playback::{Command, Phase, Playback, Status, inspect_audio, seek_position};
use mundane_ui::{PlaybackControls, Theme, TransportAction, WindowHeader};
use queue::Queue;
use std::path::PathBuf;

struct Music {
    queue: Queue,
    player: Playback,
    status: Status,
    request: u64,
    list: FocusHandle,
    scroll: UniformListScrollHandle,
    header: WindowHeader,
    transport: PlaybackControls,
    add_button: FocusHandle,
    buttons: [FocusHandle; 6],
    theme: Theme,
    title: String,
    choosing: bool,
    notice: Option<String>,
}
impl Music {
    fn new(player: Playback, w: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = cx.focus_handle().tab_stop(true);
        list.focus(w, cx);
        w.set_window_title("Music");
        Self {
            queue: Default::default(),
            player,
            status: Default::default(),
            request: 0,
            list,
            scroll: UniformListScrollHandle::new(),
            header: WindowHeader::new(cx),
            transport: PlaybackControls::new(cx),
            add_button: cx.focus_handle().tab_stop(true),
            buttons: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            theme: Theme::current(),
            title: "Music".into(),
            choosing: false,
            notice: None,
        }
    }
    fn append(&mut self, paths: Vec<PathBuf>, w: &mut Window, cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        self.notice = None;
        let mut added = Vec::new();
        for path in paths {
            added.push((self.queue.append(path.clone()), path));
        }
        if self.queue.current.is_none() {
            self.play(added[0].0, cx)
        }
        let executor = cx.background_executor().clone();
        cx.spawn_in(w, async move |view, cx| {
            for (id, path) in added {
                let info = executor.spawn(async move { inspect_audio(&path) }).await;
                if view
                    .update_in(cx, |this, w, cx| {
                        if let Some(t) = this.queue.tracks.iter_mut().find(|t| t.id == id) {
                            match info {
                                Ok(info) => t.info = info,
                                Err(e) => t.error = Some(format!("{e:#}")),
                            }
                            this.window_title(w);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn window_title(&mut self, w: &mut Window) {
        let title = self.queue.position(self.queue.current).map_or_else(
            || "Music".into(),
            |i| format!("{} — Music", self.queue.tracks[i].title()),
        );
        if self.title != title {
            w.set_window_title(&title);
            self.title = title
        }
    }
    fn play(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(i) = self.queue.position(Some(id)) else {
            return;
        };
        self.request += 1;
        self.queue.current = Some(id);
        self.queue.selected = Some(id);
        self.scroll.scroll_to_item(i, ScrollStrategy::Nearest);
        self.status = Status::default();
        self.notice = None;
        self.player.send(Command::OpenAudio {
            path: self.queue.tracks[i].path.clone(),
            request: self.request,
        });
        cx.notify()
    }
    fn watch(&self, w: &mut Window, cx: &mut Context<Self>) {
        let events = self.player.events();
        cx.spawn_in(w, async move |v, cx| {
            while events.recv().await.is_ok() {
                if v.update_in(cx, |this, w, cx| this.receive(w, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }
    fn receive(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let updated = self.player.take().status;
        if self.queue.current.is_none() || updated.audio_request != Some(self.request) {
            return;
        }
        let ended = updated.phase == Phase::Ended && self.status.phase != Phase::Ended;
        self.status = updated;
        if ended && let Some(next) = self.queue.neighbor(self.queue.current, 1) {
            self.play(next, cx)
        }
        self.window_title(w);
        cx.notify();
    }
    fn choose(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.choosing {
            return;
        }
        self.choosing = true;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add tracks".into()),
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = choice.await;
            let _ = v.update_in(cx, |this, w, cx| {
                this.choosing = false;
                match result {
                    Ok(Ok(Some(paths))) => this.append(paths, w, cx),
                    Ok(Ok(None)) => {}
                    Ok(Err(e)) => this.notice = Some(format!("File chooser unavailable. {e}")),
                    Err(e) => this.notice = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn skip(&mut self, by: isize, cx: &mut Context<Self>) {
        if let Some(id) = self.queue.neighbor(self.queue.current, by) {
            self.play(id, cx)
        }
    }
    fn select(&mut self, id: u64, w: &mut Window, cx: &mut Context<Self>) {
        self.queue.selected = Some(id);
        self.list.focus(w, cx);
        self.reveal(cx)
    }
    fn reveal(&self, cx: &mut Context<Self>) {
        if let Some(i) = self.queue.position(self.queue.selected) {
            self.scroll.scroll_to_item(i, ScrollStrategy::Nearest)
        }
        cx.notify()
    }
    fn remove(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let old = self.queue.current;
        if self
            .queue
            .remove_selected()
            .is_some_and(|id| Some(id) == old)
        {
            if let Some(next) = self.queue.selected {
                self.play(next, cx)
            } else {
                self.queue.current = None;
                self.player.send(Command::Stop);
                self.status = Status::default();
            }
        }
        self.window_title(w);
        self.list.focus(w, cx);
        self.reveal(cx)
    }
    fn transport(&self, action: TransportAction) {
        if !matches!(
            self.status.phase,
            Phase::Playing | Phase::Paused | Phase::Ended
        ) {
            return;
        }
        let command = match action {
            TransportAction::Toggle => Command::Toggle,
            TransportAction::Step(ms) => Command::Step(ms),
            TransportAction::Seek(f) => {
                let Some(d) = self.status.duration else {
                    return;
                };
                Command::Seek(seek_position(f, d))
            }
        };
        self.player.send(command)
    }
}
fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Music requires a Wayland session.");
        std::process::exit(1)
    }
    let paths = std::env::args_os().skip(1).map(PathBuf::from).collect();
    Application::with_platform(gpui_linux::current_platform(false)).run(move |cx| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(790.), px(520.)), cx);
        if let Err(e) = cx.open_window(
            WindowOptions {
                app_id: Some("org.joelinux.mundane.Music".into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |w, cx| {
                cx.new(|cx| {
                    let mut music = Music::new(Playback::start(), w, cx);
                    music.watch(w, cx);
                    music.append(paths, w, cx);
                    music
                })
            },
        ) {
            eprintln!("Could not open Music. {e}");
            std::process::exit(1)
        }
    })
}
