#[cfg(test)]
mod tests;

use gpui::{prelude::*, *};
use mundane_services::playback::{Command, Phase, Playback, Status, seek_position};
use mundane_ui::{
    ControlRole, PlaybackControls, Role, Theme, TransportAction, TransportState, WindowHeader,
    button, focus_ring, playback_time,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

const APP_ID: &str = "org.joelinux.mundane.Video";
struct Video {
    player: Playback,
    status: Status,
    frame: Option<Arc<RenderImage>>,
    dimensions: (u32, u32),
    header: WindowHeader,
    transport: PlaybackControls,
    screen: FocusHandle,
    open_button: FocusHandle,
    choosing: bool,
    notice: Option<String>,
}
impl Video {
    fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        let screen = cx.focus_handle().tab_stop(true);
        screen.focus(w, cx);
        w.set_window_title("Video");
        let player = Playback::start();
        Self {
            player,
            status: Default::default(),
            frame: None,
            dimensions: (1, 1),
            header: WindowHeader::new(cx),
            transport: PlaybackControls::new(cx),
            screen,
            open_button: cx.focus_handle().tab_stop(true),
            choosing: false,
            notice: None,
        }
    }
    fn watch(&self, w: &mut Window, cx: &mut Context<Self>) {
        let events = self.player.events();
        cx.spawn_in(w, async move |view, cx| {
            while events.recv().await.is_ok() {
                if view
                    .update_in(cx, |this, w, cx| this.receive(w, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
    fn name(&self) -> String {
        self.status
            .file
            .as_deref()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
    fn receive(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let update = self.player.take();
        let changed = self.status.file != update.status.file;
        self.status = update.status;
        if changed {
            w.set_window_title(&format!("{} — Video", self.name()));
            self.screen.focus(w, cx);
        }
        if let Some(frame) = update.frame {
            self.dimensions = (frame.width, frame.height);
            let data = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels)
                .expect("validated BGRA frame");
            let next = Arc::new(RenderImage::new([image::Frame::new(data)]));
            if let Some(old) = self.frame.replace(next) {
                let _ = w.drop_image(old);
            }
        } else if changed && let Some(old) = self.frame.take() {
            let _ = w.drop_image(old);
        }
        cx.notify();
    }
    fn open(&mut self, path: PathBuf) {
        self.notice = None;
        self.player.send(Command::Open(path));
    }
    fn choose(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.choosing {
            return;
        }
        self.choosing = true;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(w, async move |view, cx| {
            let result = choice.await;
            let _ = view.update(cx, |this, cx| {
                this.choosing = false;
                match result {
                    Ok(Ok(paths)) => {
                        if let Some(path) = paths.and_then(|p| p.into_iter().next()) {
                            this.open(path);
                        }
                    }
                    Ok(Err(e)) => this.notice = Some(format!("File chooser unavailable. {e}")),
                    Err(e) => this.notice = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn transport(&self, cmd: TransportAction) {
        if !matches!(
            self.status.phase,
            Phase::Playing | Phase::Paused | Phase::Ended
        ) {
            return;
        }
        let command = match cmd {
            TransportAction::Toggle => Command::Toggle,
            TransportAction::Step(ms) => Command::Step(ms),
            TransportAction::Seek(f) => {
                let Some(duration) = self.status.duration else {
                    return;
                };
                Command::Seek(seek_position(f, duration))
            }
        };
        self.player.send(command);
    }
    fn keyboard(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        match (e.keystroke.key.as_str(), e.keystroke.modifiers.control) {
            ("o", true) => self.choose(w, cx),
            ("w", true) => w.remove_window(),
            ("space" | "enter", false) if self.screen.is_focused(w) => {
                self.transport(TransportAction::Toggle)
            }
            ("left", false) => self.transport(TransportAction::Step(-5000)),
            ("right", false) => self.transport(TransportAction::Step(5000)),
            ("home", false) => self.transport(TransportAction::Seek(0.)),
            ("end", false) => self.transport(TransportAction::Seek(1.)),
            ("f6", false) => self.open_button.focus(w, cx),
            ("escape", false) => self.screen.focus(w, cx),
            ("tab", false) if e.keystroke.modifiers.shift => w.focus_prev(cx),
            ("tab", false) => w.focus_next(cx),
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation();
    }
}

impl Render for Video {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let view = cx.weak_entity();
        let open = button("Open", &self.open_button, theme, move |w, cx| {
            let _ = view.update(cx, |this, cx| this.choose(w, cx));
        });
        let state = TransportState {
            available: matches!(
                self.status.phase,
                Phase::Playing | Phase::Paused | Phase::Ended
            ),
            playing: self.status.phase == Phase::Playing,
            ended: self.status.phase == Phase::Ended,
            position: self.status.position,
            duration: self.status.duration,
        };
        let phase = match self.status.phase {
            Phase::Empty => "",
            Phase::Playing => "Playing",
            Phase::Paused => "Paused",
            Phase::Ended => "Ended",
            Phase::Failed => "Playback failed",
        };
        let name = self.name();
        let view = cx.weak_entity();
        let screen = focus_ring(
            Role::Video.control(
                "screen",
                if name.is_empty() {
                    "Video".into()
                } else {
                    name.clone()
                },
            ),
            &self.screen,
            theme,
        )
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, w, cx| this.screen.focus(w, cx)),
        );
        let screen = if let Some(image) = self.frame.clone() {
            let (width, height) = self.dimensions;
            let paint_view = cx.weak_entity();
            screen.child(
                canvas(
                    |_, _, _| (),
                    move |b, _, w, cx| {
                        let scale = (f32::from(b.size.width) / width as f32)
                            .min(f32::from(b.size.height) / height as f32);
                        let dim = size(px(width as f32 * scale), px(height as f32 * scale));
                        let dest =
                            Bounds::new(b.center() - point(dim.width / 2., dim.height / 2.), dim);
                        if let Err(err) =
                            w.paint_image(b, dest, Default::default(), image, 0, false)
                        {
                            w.defer(cx, move |_, cx| {
                                let _ = paint_view.update(cx, |video, cx| {
                                    if video.notice.is_none() {
                                        if video.status.phase == Phase::Playing {
                                            video.transport(TransportAction::Toggle);
                                        }
                                        video.notice =
                                            Some(format!("Could not display video frame. {err}"));
                                        cx.notify();
                                    }
                                });
                            });
                        }
                    },
                )
                .size_full(),
            )
        } else {
            screen.child(
                if self.status.phase == Phase::Failed || self.status.error.is_some() {
                    "Video unavailable"
                } else {
                    "Open a WebM video"
                },
            )
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .on_key_down(cx.listener(Self::keyboard))
            .child(self.header.render("Video", open, &self.screen, theme, w))
            .child(screen)
            .when(!name.is_empty(), |el| {
                el.child(
                    Role::Status
                        .control("status", "Playback status")
                        .aria_description(format!(
                            "{name} · {phase} · {}",
                            playback_time(self.status.position)
                        ))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_3()
                        .pt_2()
                        .bg(rgb(theme.panel))
                        .child(
                            div()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(theme.secondary))
                                .child(phase),
                        ),
                )
            })
            .child(self.transport.render(state, theme, move |cmd, _, cx| {
                let _ = view.update(cx, |this, _| this.transport(cmd));
            }))
            .when_some(
                self.notice.clone().or_else(|| self.status.error.clone()),
                |el, msg| {
                    el.child(
                        Role::Status
                            .control("error", msg.clone())
                            .px_3()
                            .py_1()
                            .child(msg),
                    )
                },
            )
    }
}
fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Video requires a Wayland session.");
        std::process::exit(1);
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    mundane_ui::run(move |cx| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(800.), px(520.)), cx);
        if let Err(e) = cx.open_window(
            WindowOptions {
                app_id: Some(APP_ID.into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let mut viewer = Video::new(window, cx);
                    viewer.watch(window, cx);
                    if let Some(path) = path {
                        viewer.open(path);
                    }
                    viewer
                })
            },
        ) {
            eprintln!("Could not open Video. {e}");
            std::process::exit(1);
        }
    });
}
