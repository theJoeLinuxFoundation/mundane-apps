use mundane_ui::ControlRole as _;
mod dialogs;
mod listing;
#[cfg(test)]
mod tests;

use dialogs::{Dialog, Prompt};
use gpui::{prelude::*, *};
use listing::Previews;
use mundane_services::files::{self, Entry, Transfer};
use mundane_ui::{Theme, WindowHeader, button, focus_ring};
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

const APP_ID: &str = "org.joelinux.mundane.Files";
const CUT: &str = "org.joelinux.mundane.Files.cut";
#[derive(Clone)]
struct Paste {
    source: PathBuf,
    dest: PathBuf,
    how: Transfer,
}
enum PasteAction {
    New,
    KeepBoth,
    Replace(files::Conflict),
}
enum Job {
    Open(PathBuf),
    Rename(PathBuf, OsString),
    Paste(Paste, PasteAction),
    Trash(PathBuf),
}
enum Outcome {
    Changed(Option<PathBuf>),
    Opened,
    Conflict(Paste, files::Conflict),
}

impl Job {
    async fn run(self) -> Result<Outcome, String> {
        let result = match self {
            Self::Open(path) => return files::open(&path).await.map(|()| Outcome::Opened),
            Self::Rename(path, name) => {
                files::rename(&path, &name).map(|p| Outcome::Changed(Some(p)))
            }
            Self::Trash(path) => files::trash(&path).map(|()| Outcome::Changed(None)),
            Self::Paste(paste, PasteAction::KeepBoth) => {
                files::keep_both(&paste.source, &paste.dest, paste.how)
                    .map(|p| Outcome::Changed(Some(p)))
            }
            Self::Paste(paste, PasteAction::Replace(review)) => {
                files::replace(&paste.source, &paste.dest, paste.how, &review)
                    .map(|()| Outcome::Changed(Some(paste.dest)))
            }
            Self::Paste(paste, PasteAction::New) => {
                match files::transfer(&paste.source, &paste.dest, paste.how) {
                    Ok(()) => Ok(Outcome::Changed(Some(paste.dest))),
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                        files::Conflict::inspect(&paste.source, &paste.dest)
                            .map(|info| Outcome::Conflict(paste, info))
                    }
                    Err(e) => Err(e),
                }
            }
        };
        result.map_err(|e| e.to_string())
    }
}

#[derive(Clone, Copy)]
enum Command {
    Location,
    Up,
    Copy,
    Cut,
    Paste,
    Rename,
    Trash,
}
impl Command {
    const TOOLS: [Self; 6] = [
        Self::Up,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::Rename,
        Self::Trash,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Location => "Location",
            Self::Up => "Up",
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::Rename => "Rename",
            Self::Trash => "Trash",
        }
    }
}

struct Files {
    path: PathBuf,
    entries: Vec<Entry>,
    selected: Option<usize>,
    generation: u64,
    loading: bool,
    working: bool,
    show_hidden: bool,
    message: Option<String>,
    header: WindowHeader,
    content: mundane_ui::ApplicationContent,
    list_focus: FocusHandle,
    location_focus: FocusHandle,
    tools: [FocusHandle; 6],
    scroll: UniformListScrollHandle,
    dialog: Option<Dialog>,
    previews: Previews,
}

impl Files {
    fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list_focus = cx.focus_handle().tab_stop(true);
        list_focus.focus(window, cx);
        let mut this = Self {
            path: path.clone(),
            entries: vec![],
            selected: None,
            generation: 0,
            loading: false,
            working: false,
            show_hidden: false,
            message: None,
            content: Default::default(),
            header: WindowHeader::new(cx),
            list_focus,
            location_focus: cx.focus_handle().tab_stop(true),
            tools: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            scroll: UniformListScrollHandle::new(),
            dialog: None,
            previews: Previews::default(),
        };
        this.load(path, None, cx);
        this
    }

    fn load(&mut self, path: PathBuf, prefer: Option<PathBuf>, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        let hidden = self.show_hidden;
        let requested = path.clone();
        self.previews = Previews::default();
        self.loading = true;
        self.message = None;
        let scan = cx.background_executor().spawn(async move {
            files::list(&requested).map(|mut entries| {
                if !hidden {
                    entries.retain(|entry| !entry.name.as_encoded_bytes().starts_with(b"."));
                }
                entries
            })
        });
        cx.spawn(async move |view, cx| {
            let result = scan.await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(entries) => {
                        this.selected = prefer
                            .as_ref()
                            .and_then(|p| entries.iter().position(|entry| &entry.path == p))
                            .or_else(|| (!entries.is_empty()).then_some(0));
                        this.entries = entries;
                        this.path = path;
                        this.scroll = UniformListScrollHandle::new();
                        if let Some(i) = this.selected {
                            this.scroll.scroll_to_item(i, ScrollStrategy::Nearest);
                        }
                    }
                    Err(e) => {
                        this.message = Some(format!("Could not open {}. {e}", path.display()))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn execute(&mut self, job: Job, window: &mut Window, cx: &mut Context<Self>) {
        if self.working || self.loading {
            return;
        }
        let cut_source = match &job {
            Job::Paste(p, _) if p.how == Transfer::Move => Some(p.source.clone()),
            _ => None,
        };
        self.working = true;
        self.message = None;
        let work = cx.background_executor().spawn(job.run());
        cx.spawn_in(window, async move |view, cx| {
            let result = work.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.working = false;
                match result {
                    Ok(Outcome::Changed(selection)) => {
                        if cut_source.is_some()
                            && cx
                                .read_from_clipboard()
                                .as_ref()
                                .and_then(clipboard_path)
                                .is_some_and(|(p, how)| {
                                    Some(p) == cut_source && how == Transfer::Move
                                })
                        {
                            cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
                        }
                        this.load(this.path.clone(), selection, cx);
                    }
                    Ok(Outcome::Opened) => {}
                    Ok(Outcome::Conflict(p, info)) => {
                        this.prompt(Prompt::Conflict(p, info), window, cx)
                    }
                    Err(e) => this.message = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn selected(&self) -> Option<&Entry> {
        self.selected.and_then(|i| self.entries.get(i))
    }
    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.working {
            return;
        }
        if let Some(entry) = self.selected() {
            if entry.folder {
                self.load(entry.path.clone(), None, cx);
            } else {
                self.execute(Job::Open(entry.path.clone()), window, cx);
            }
        }
    }

    fn command(&mut self, cmd: Command, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() || self.working || self.loading {
            return;
        }
        match cmd {
            Command::Location => self.prompt(Prompt::Location, window, cx),
            Command::Up => {
                if let Some(parent) = self.path.parent() {
                    self.load(parent.to_owned(), Some(self.path.clone()), cx);
                }
            }
            Command::Copy | Command::Cut => {
                if let Some(e) = self.selected()
                    && let Ok(uri) = url::Url::from_file_path(&e.path)
                {
                    let cut = matches!(cmd, Command::Cut);
                    cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
                        uri.to_string(),
                        if cut { CUT.into() } else { String::new() },
                    ));
                    self.message = Some(format!(
                        "{} ready to {}",
                        e.name.to_string_lossy(),
                        if cut { "move" } else { "copy" }
                    ));
                }
            }
            Command::Paste => {
                if let Some((source, how)) =
                    cx.read_from_clipboard().as_ref().and_then(clipboard_path)
                    && let Some(name) = source.file_name()
                {
                    let dest = self.path.join(name);
                    if source != dest || how != Transfer::Move {
                        self.execute(
                            Job::Paste(Paste { source, dest, how }, PasteAction::New),
                            window,
                            cx,
                        );
                    }
                } else {
                    self.message = Some("Copy or cut a local item first.".into());
                }
            }
            Command::Rename => {
                if let Some(entry) = self.selected() {
                    self.prompt(Prompt::Rename(entry.path.clone()), window, cx);
                }
            }
            Command::Trash => {
                if let Some(entry) = self.selected() {
                    self.prompt(Prompt::Trash(entry.path.clone()), window, cx);
                }
            }
        }
        cx.notify();
    }

    fn keys(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let cmd = match (key.key.as_str(), key.modifiers.control, key.modifiers.alt) {
            ("l", true, false) => Some(Command::Location),
            ("c", true, false) => Some(Command::Copy),
            ("x", true, false) => Some(Command::Cut),
            ("v", true, false) => Some(Command::Paste),
            ("f2", false, false) => Some(Command::Rename),
            ("delete", false, false) => Some(Command::Trash),
            ("up", false, true) | ("backspace", false, false) => Some(Command::Up),
            _ => None,
        };
        if let Some(cmd) = cmd {
            self.command(cmd, window, cx);
        } else {
            match key.key.as_str() {
                "enter" => self.activate(window, cx),
                "up" | "down" | "home" | "end" if !self.entries.is_empty() => {
                    let current = self.selected.unwrap_or_default();
                    let last = self.entries.len() - 1;
                    let next = match key.key.as_str() {
                        "up" => current.saturating_sub(1),
                        "down" => (current + 1).min(last),
                        "home" => 0,
                        _ => last,
                    };
                    self.selected = Some(next);
                    self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
                    self.message = None;
                }
                "f6" => self.location_focus.focus(window, cx),
                "f5" if !self.working => self.load(
                    self.path.clone(),
                    self.selected().map(|e| e.path.clone()),
                    cx,
                ),
                "h" if key.modifiers.control && !self.working => {
                    self.show_hidden = !self.show_hidden;
                    self.load(self.path.clone(), None, cx);
                }
                _ => return,
            }
        }
        cx.stop_propagation();
        window.prevent_default();
        cx.notify();
    }

    fn tool(&self, cmd: Command, focus: &FocusHandle, cx: &Context<Self>) -> Stateful<Div> {
        let view = cx.weak_entity();
        let disabled = self.working
            || self.loading
            || self.dialog.is_some()
            || match cmd {
                Command::Up => self.path.parent().is_none(),
                Command::Copy | Command::Cut | Command::Trash | Command::Rename => {
                    self.selected().is_none()
                }
                _ => false,
            };
        let tool = button(cmd.label(), focus, Theme::current(cx), move |window, cx| {
            let _ = view.update(cx, |this, cx| this.command(cmd, window, cx));
        });
        mundane_ui::unavailable(tool, disabled)
    }
}

fn clipboard_path(item: &ClipboardItem) -> Option<(PathBuf, Transfer)> {
    if let [ClipboardEntry::ExternalPaths(paths)] = item.entries.as_slice()
        && let [path] = paths.paths()
    {
        return Some((path.clone(), Transfer::Copy));
    }
    let uri = url::Url::parse(item.text()?.trim()).ok()?;
    let mode = if item.metadata().is_some_and(|s| s == CUT) {
        Transfer::Move
    } else {
        Transfer::Copy
    };
    Some((uri.to_file_path().ok()?, mode))
}

fn location_label(path: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(rest) = path.strip_prefix(home)
    {
        return if rest.as_os_str().is_empty() {
            "Home".into()
        } else {
            format!("Home / {}", rest.display())
        };
    }
    path.display().to_string()
}

impl Render for Files {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let tools = div().flex().flex_wrap().gap_1().children(
            Command::TOOLS
                .into_iter()
                .zip(&self.tools)
                .map(|(cmd, f)| self.tool(cmd, f, cx)),
        );
        let list = uniform_list("entries", self.entries.len(), cx.processor(Self::rows))
            .track_scroll(&self.scroll)
            .size_full();
        let footer = if self.loading {
            "Loading folder…".to_string()
        } else if self.working {
            "Working…".into()
        } else {
            self.message.clone().unwrap_or_else(|| {
                format!(
                    "{} {}",
                    self.entries.len(),
                    if self.entries.len() == 1 {
                        "item"
                    } else {
                        "items"
                    }
                )
            })
        };
        let body = self
            .content
            .render(self.dialog.is_some())
            .size_full()
            .flex()
            .flex_col()
            .font_family("DejaVu Sans")
            .bg(rgb(theme.surface))
            .text_color(rgb(theme.text))
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "w" && event.keystroke.modifiers.control {
                    window.remove_window();
                    cx.stop_propagation();
                } else if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx)
                    } else {
                        window.focus_next(cx)
                    };
                    cx.stop_propagation();
                }
            })
            .child(self.header.render(
                "Files",
                self.tool(Command::Location, &self.location_focus, cx),
                &self.list_focus,
                theme,
                window,
            ))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .bg(rgb(theme.panel))
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(location_label(&self.path)),
                    )
                    .child(tools),
            )
            .child(
                focus_ring(
                    mundane_ui::Role::ListBox
                        .control("folder", "Folder contents")
                        .aria_size_of_set(self.entries.len()),
                    &self.list_focus,
                    theme,
                )
                .min_h_0()
                .flex_1()
                .overflow_hidden()
                .on_key_down(cx.listener(Self::keys))
                .when(
                    self.entries.is_empty() && !self.loading && self.message.is_none(),
                    |el| el.child(div().p_3().child("This folder is empty.")),
                )
                .child(list),
            )
            .child(
                mundane_ui::Role::Status
                    .control("file-status", footer.clone())
                    .flex_none()
                    .px_3()
                    .py_1()
                    .text_sm()
                    .text_color(rgb(theme.secondary))
                    .child(footer),
            );
        div()
            .relative()
            .size_full()
            .font_family("DejaVu Sans")
            .text_color(rgb(theme.text))
            .child(body)
            .children(self.dialog.as_ref().map(|d| self.dialog_view(d, cx)))
    }
}

fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Files requires a Wayland session.");
        std::process::exit(1);
    }
    let path = std::env::args_os()
        .nth(1)
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let path = std::path::absolute(path).unwrap_or_else(|e| {
        eprintln!("Could not resolve folder: {e}");
        std::process::exit(1)
    });
    mundane_ui::run(move |cx| {
        mundane_ui::bind_text_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(820.), px(540.)), cx);
        if let Err(e) = cx.open_window(
            WindowOptions {
                app_id: Some(APP_ID.into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Files");
                cx.new(|cx| Files::new(path, window, cx))
            },
        ) {
            eprintln!("Could not open Files: {e}");
            std::process::exit(1);
        }
    });
}
