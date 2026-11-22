mod gallery;
mod picture;
#[cfg(test)]
mod tests;
mod view;

use gpui::{prelude::*, *};
use mundane_ui::{Theme, WindowHeader};
use std::{path::PathBuf, sync::Arc};

const APP_ID: &str = "org.joelinux.mundane.ImageViewer";
const ZOOM: [f32; 11] = [0.25, 0.5, 0.75, 1., 1.25, 1.5, 2., 3., 4., 6., 8.];
const FIT: usize = 3;

struct Picture {
    name: String,
    width: u32,
    height: u32,
    pixels: Arc<RenderImage>,
}

impl Picture {
    fn open(path: PathBuf) -> anyhow::Result<Self> {
        let mut pixels = picture::open(&path)?;
        let (width, height) = pixels.dimensions();
        for pixel in pixels.pixels_mut() {
            pixel.0.swap(0, 2); // GPUI uploads BGRA.
        }
        Ok(Self {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            width,
            height,
            pixels: Arc::new(RenderImage::new([image::Frame::new(pixels)])),
        })
    }
}

#[derive(Copy, Clone)]
enum Command {
    Open,
    Previous,
    Next,
    Smaller,
    Larger,
    Fit,
}

struct Viewer {
    picture: Option<Picture>,
    zoom: usize,
    // Fraction of the available travel in each direction, from -1 to 1.
    pan: Point<f32>,
    busy: bool,
    error: Option<String>,
    header: WindowHeader,
    tools: [FocusHandle; 6],
    gallery: gallery::Gallery,
    image_focus: FocusHandle,
}

impl Viewer {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle().tab_stop(true);
        focus.focus(window, cx);
        window.set_window_title("Image Viewer");
        Self {
            picture: None,
            gallery: Default::default(),
            zoom: FIT,
            pan: Default::default(),
            busy: false,
            error: None,
            header: WindowHeader::new(cx),
            tools: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            image_focus: focus,
        }
    }

    fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.load(
            move || {
                let gallery = gallery::Gallery::for_path(&path);
                Ok((Picture::open(path)?, gallery))
            },
            window,
            cx,
        );
    }
    fn load(
        &mut self,
        read: impl FnOnce() -> anyhow::Result<(Picture, gallery::Gallery)> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.error = None;
        let task = cx.background_executor().spawn(async move { read() });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok((pic, gallery)) => {
                        this.gallery = gallery;
                        window.set_window_title(&format!("{} — Image Viewer", pic.name));
                        if let Some(old) = this.picture.replace(pic) {
                            let _ = window.drop_image(old.pixels);
                        }
                        this.zoom = FIT;
                        this.pan = Default::default();
                        this.image_focus.focus(window, cx);
                    }
                    Err(err) => this.error = Some(format!("Could not open image. {err}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = choice.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(Ok(paths)) => {
                        if let Some(path) = paths.and_then(|paths| paths.into_iter().next()) {
                            this.open(path, window, cx);
                        }
                    }
                    Ok(Err(err)) => this.error = Some(format!("File chooser unavailable. {err}")),
                    Err(err) => this.error = Some(err.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn enabled(&self, cmd: Command) -> bool {
        if self.busy {
            return false;
        }
        match cmd {
            Command::Open => true,
            Command::Previous => self.gallery.neighbour(-1).is_some(),
            Command::Next => self.gallery.neighbour(1).is_some(),
            Command::Smaller => self.picture.is_some() && self.zoom > 0,
            Command::Larger => self.picture.is_some() && self.zoom + 1 < ZOOM.len(),
            Command::Fit => self.picture.is_some(),
        }
    }

    fn command(&mut self, cmd: Command, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled(cmd) {
            return;
        }
        match cmd {
            Command::Open => self.choose(window, cx),
            Command::Previous | Command::Next => {
                let gallery = self.gallery.clone();
                self.load(
                    move || {
                        gallery.step(if matches!(cmd, Command::Previous) {
                            -1
                        } else {
                            1
                        })
                    },
                    window,
                    cx,
                );
            }
            Command::Smaller => self.zoom -= 1,
            Command::Larger => self.zoom += 1,
            Command::Fit => self.zoom = FIT,
        }
        if self.zoom <= FIT {
            self.pan = Default::default()
        }
        cx.notify();
    }
}
fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Image Viewer requires a Wayland session.");
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
                    let mut viewer = Viewer::new(window, cx);
                    if let Some(path) = path {
                        viewer.open(path, window, cx);
                    }
                    viewer
                })
            },
        ) {
            eprintln!("Could not open Image Viewer. {e}");
            std::process::exit(1);
        }
    });
}
