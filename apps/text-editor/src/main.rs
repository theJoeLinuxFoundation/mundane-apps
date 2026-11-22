mod document;
mod editing;
mod indent;
mod printing;
mod state;
mod syntax;
mod tabs;
use tabs::Page;
#[cfg(test)]
mod tests;
mod view;

use document::Document;
use gpui::{prelude::*, *};
use mundane_ui::{TextField, Theme, WindowHeader};
use std::path::{Path, PathBuf};

const APP_ID: &str = "org.joelinux.mundane.TextEditor";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    CloseTab,
    CloseWindow,
}

#[derive(Clone, Copy)]
enum Command {
    Print,
    Reopen,
    Recent,
    Replace,
    ReplaceOne,
    ReplaceAll,
    New,
    CloseTab,
    Line,
    Jump,
    Open,
    Find,
    Save,
    SaveAs,
    Next,
    Done,
    Cancel,
    Discard,
    ConfirmSave,
}
impl Command {
    fn label(self) -> &'static str {
        match self {
            Self::Print => "Print",
            Self::Reopen => "Reopen closed tab",
            Self::Recent => "Recent",
            Self::Replace => "Find and replace",
            Self::ReplaceOne => "Replace",
            Self::ReplaceAll => "Replace all",
            Self::New => "New tab",
            Self::Line => "Go to line",
            Self::Jump => "Go",
            Self::CloseTab => "Close tab",
            Self::Open => "Open",
            Self::Find => "Find",
            Self::Save | Self::ConfirmSave => "Save",
            Self::SaveAs => "Save As",
            Self::Next => "Next",
            Self::Done => "Done",
            Self::Cancel => "Cancel",
            Self::Discard => "Discard",
        }
    }
}

struct Confirmation {
    pending: Pending,
    return_focus: FocusHandle,
    buttons: [FocusHandle; 3],
}

struct Editor {
    indent: indent::Indent,
    indent_focus: FocusHandle,
    print_focus: FocusHandle,
    reopen_focus: FocusHandle,
    closed: Vec<PathBuf>,
    recent: mundane_ui::BrowserList,
    showing_recent: bool,
    recent_focus: FocusHandle,
    replacement: Entity<TextField>,
    replacing: bool,
    replace_tools: [FocusHandle; 3],
    result: Option<String>,
    pages: Vec<Page>,
    active: usize,
    next_id: usize,
    scroll: ScrollHandle,
    close_focus: FocusHandle,
    new_focus: FocusHandle,
    line_focus: FocusHandle,
    going: bool,
    line: Entity<TextField>,
    query: Entity<TextField>,
    searching: bool,
    last_query: SharedString,
    header: WindowHeader,
    content: mundane_ui::ApplicationContent,
    tools: [FocusHandle; 4],
    find_buttons: [FocusHandle; 2],
    confirmation: Option<Confirmation>,
    busy: Option<&'static str>,
    busy_focus: FocusHandle,
    error: Option<String>,
}

impl Editor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let theme = Theme::current(cx);
        let page = Page::new(1, cx);
        let query = cx.new(|cx| TextField::new("Find in document", theme, cx));
        cx.observe(&query, |this, query, cx| {
            let term = query.read(cx).snapshot();
            if this.searching && term != this.last_query {
                this.last_query = term;
                this.find_match(false, cx);
            }
        })
        .detach();
        page.body.read(cx).focus_handle(cx).focus(window, cx);
        let view = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            view.update(cx, |this, cx| {
                this.can_leave(Pending::CloseWindow, window, cx)
            })
            .unwrap_or(true)
        });
        let view = cx.weak_entity();
        let header = WindowHeader::new(cx).on_close(move |window, cx| {
            let _ = view.update(cx, |this, cx| {
                this.request(Pending::CloseWindow, window, cx)
            });
        });
        let setting = indent::Indent::load();
        let mut editor = Self {
            indent: setting.as_ref().copied().unwrap_or_default(),
            indent_focus: cx.focus_handle().tab_stop(true),
            print_focus: cx.focus_handle().tab_stop(true),
            reopen_focus: cx.focus_handle().tab_stop(true),
            closed: vec![],
            recent: mundane_ui::BrowserList::new(cx),
            showing_recent: false,
            recent_focus: cx.focus_handle().tab_stop(true),
            replacement: cx.new(|cx| TextField::new("Replacement text", theme, cx)),
            replacing: false,
            replace_tools: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            result: None,
            pages: vec![page],
            active: 0,
            next_id: 2,
            scroll: ScrollHandle::new(),
            close_focus: cx.focus_handle().tab_stop(true),
            line_focus: cx.focus_handle().tab_stop(true),
            new_focus: cx.focus_handle().tab_stop(true),
            going: false,
            line: cx.new(|cx| TextField::new("Line number", theme, cx)),
            query,
            searching: false,
            last_query: "".into(),
            content: Default::default(),
            header,
            tools: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            find_buttons: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            confirmation: None,
            busy: None,
            busy_focus: cx.focus_handle(),
            error: None,
        };
        editor.restore(window, cx);
        if let Err(e) = setting {
            editor.error = Some(format!("Could not load indentation. {e}"));
        }
        editor
    }

    fn dirty(&self, cx: &App) -> bool {
        self.page().dirty(cx)
    }

    fn name(&self) -> String {
        self.page().name()
    }

    fn focus_body(&self, window: &mut Window, cx: &mut App) {
        self.page().body.read(cx).focus_handle(cx).focus(window, cx);
    }

    fn can_leave(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.busy.is_some() || self.confirmation.is_some() {
            return false;
        }
        if !self.dirty(cx) {
            if pending == Pending::CloseWindow
                && let Some(i) = self.pages.iter().position(|p| p.dirty(cx))
            {
                self.switch(i, window, cx);
            } else {
                if pending == Pending::CloseWindow
                    && let Err(e) = self.pages.iter().try_for_each(Page::clear_draft)
                {
                    self.error = Some(format!("Could not clear the recovery copy. {e}"));
                    cx.notify();
                    return false;
                }
                return true;
            }
        }
        let confirm = Confirmation {
            pending,
            return_focus: window
                .focused(cx)
                .unwrap_or_else(|| self.page().body.read(cx).focus_handle(cx)),
            buttons: std::array::from_fn(|_| cx.focus_handle()),
        };
        confirm.buttons[0].focus(window, cx);
        self.confirmation = Some(confirm);
        cx.notify();
        false
    }

    fn request(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        if self.can_leave(pending, window, cx) {
            self.proceed(pending, window, cx);
        }
    }

    fn proceed(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.page().clear_draft() {
            self.error = Some(format!("Could not clear the recovery copy. {e}"));
            cx.notify();
            return;
        }
        if self.pages.len() == 1 {
            window.remove_window();
            return;
        }
        let closed = self.pages.remove(self.active);
        if pending == Pending::CloseTab
            && let Some(path) = closed.document.path
        {
            self.closed.retain(|p| p != &path);
            self.closed.push(path);
            if self.closed.len() > 32 {
                self.closed.remove(0);
            }
        }
        self.switch(self.active.min(self.pages.len() - 1), window, cx);
        if pending == Pending::CloseWindow && self.can_leave(pending, window, cx) {
            window.remove_window();
        }
    }

    fn wait(&mut self, message: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = Some(message);
        self.error = None;
        self.busy_focus.focus(window, cx);
        cx.notify();
    }

    fn ready(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = None;
        self.focus_body(window, cx);
        cx.notify();
    }

    fn choose(
        &mut self,
        save: bool,
        after: Option<Pending>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wait("Choosing a file…", window, cx);
        let choice = if save {
            let dir = self
                .page()
                .document
                .path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_owned)
                .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
                .unwrap_or_else(|| PathBuf::from("/"));
            let prompt = cx.prompt_for_new_path(&dir, Some(&self.name()));
            cx.foreground_executor().spawn(prompt)
        } else {
            let prompt = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: None,
            });
            cx.foreground_executor().spawn(async move {
                prompt
                    .await
                    .map(|result| result.map(|paths| paths.and_then(|v| v.into_iter().next())))
            })
        };
        cx.spawn_in(window, async move |view, cx| {
            let choice = choice.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.ready(window, cx);
                match choice {
                    Ok(Ok(Some(path))) if save => this.write(path, after, window, cx),
                    Ok(Ok(Some(path))) => this.open(path, window, cx),
                    Ok(Ok(None)) => {}
                    Ok(Err(err)) => this.error = Some(format!("File chooser unavailable. {err}")),
                    Err(err) => this.error = Some(err.to_string()),
                }
            });
        })
        .detach();
    }

    fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self
            .pages
            .iter()
            .position(|page| page.document.path.as_ref() == Some(&path))
        {
            self.switch(i, window, cx);
            return;
        }
        self.wait("Opening file…", window, cx);
        let task = cx
            .background_executor()
            .spawn(async move { Document::open(path) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.ready(window, cx);
                match result {
                    Ok(doc) => {
                        if this.page().document.path.is_some() || this.dirty(cx) {
                            this.new_tab(window, cx);
                        }
                        this.page()
                            .body
                            .update(cx, |field, cx| field.set_document(doc.saved.clone(), cx));
                        this.searching = false;
                        this.going = false;
                        this.pages[this.active].document = doc;
                        this.file_ready(cx);
                    }
                    Err(err) => this.error = Some(format!("Could not open file. {err}")),
                }
            });
        })
        .detach();
    }

    fn save(&mut self, after: Option<Pending>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.page().document.path.clone() {
            self.write(path, after, window, cx);
        } else {
            self.choose(true, after, window, cx);
        }
    }

    fn write(
        &mut self,
        path: PathBuf,
        after: Option<Pending>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wait("Saving file…", window, cx);
        let doc = self.page().document.clone();
        let text = self.page().body.read(cx).snapshot();
        let work = cx
            .background_executor()
            .spawn(async move { doc.save(path, text) });
        cx.spawn_in(window, async move |view, cx| {
            let result = work.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.ready(window, cx);
                match result {
                    Ok(doc) => {
                        this.pages[this.active].document = doc;
                        this.file_ready(cx);
                        if let Some(action) = after {
                            this.request(action, window, cx);
                        }
                    }
                    Err(err) => this.error = Some(format!("Could not save file. {err}")),
                }
            });
        })
        .detach();
    }

    fn find_match(&mut self, advance: bool, cx: &mut Context<Self>) {
        let query = self.query.read(cx).snapshot();
        if !query.is_empty() {
            let body = self.page().body.read(cx);
            let range = body.selection();
            let start = if advance { range.end } else { range.start };
            let text = body.text();
            if let Some(offset) = text[start..]
                .find(query.as_str())
                .map(|i| i + start)
                .or_else(|| text.find(query.as_str()))
            {
                self.page().body.update(cx, |field, cx| {
                    field.select_match(offset..offset + query.len(), cx)
                });
            } else {
                self.page()
                    .body
                    .update(cx, |field, cx| field.select_match(start..start, cx));
            }
        }
        cx.notify();
    }

    fn command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        if let Some(confirm) = self.confirmation.take() {
            match command {
                Command::Discard => self.proceed(confirm.pending, window, cx),
                Command::ConfirmSave => self.save(Some(confirm.pending), window, cx),
                Command::Cancel => confirm.return_focus.focus(window, cx),
                _ => self.confirmation = Some(confirm),
            }
        } else {
            match command {
                Command::Print => self.print(window, cx),
                Command::Reopen => {
                    if let Some(path) = self.closed.pop() {
                        self.open(path, window, cx)
                    }
                }
                Command::Open => self.choose(false, None, window, cx),
                Command::Recent => self.show_recent(window, cx),
                Command::Replace => {
                    self.command(Command::Find, window, cx);
                    self.replacing = true;
                }
                Command::ReplaceOne | Command::ReplaceAll => {
                    self.replace(matches!(command, Command::ReplaceAll), window, cx)
                }
                Command::New => self.new_tab(window, cx),
                Command::CloseTab => self.request(Pending::CloseTab, window, cx),
                Command::Line => {
                    self.going = true;
                    self.searching = false;
                    self.error = None;
                    let theme = Theme::current(cx);
                    self.line = cx.new(|cx| TextField::new("Line number", theme, cx));
                    self.line.read(cx).focus_handle(cx).focus(window, cx);
                }
                Command::Jump => {
                    let number = self.line.read(cx).text().trim().parse::<usize>().ok();
                    if number
                        .is_some_and(|n| self.page().body.update(cx, |f, cx| f.go_to_line(n, cx)))
                    {
                        self.going = false;
                        self.error = None;
                        self.focus_body(window, cx);
                    } else {
                        self.error = Some(format!(
                            "Enter a line from 1 to {}.",
                            self.page().body.read(cx).line_count()
                        ));
                        self.line.read(cx).focus_handle(cx).focus(window, cx);
                    }
                }
                Command::Save => self.save(None, window, cx),
                Command::SaveAs => self.choose(true, None, window, cx),
                Command::Find => {
                    self.going = false;
                    self.searching = true;
                    self.showing_recent = false;
                    self.replacing = false;
                    self.result = None;
                    self.query.update(cx, |field, cx| {
                        field.select_match(0..field.text().len(), cx)
                    });
                    self.query.read(cx).focus_handle(cx).focus(window, cx);
                    self.find_match(false, cx);
                }
                Command::Next => self.find_match(true, cx),
                Command::Done => {
                    self.showing_recent = false;
                    self.replacing = false;
                    self.result = None;
                    self.going = false;
                    self.error = None;
                    self.searching = false;
                    self.focus_body(window, cx);
                }
                _ => {}
            }
        }
        cx.notify();
    }
}

fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Text Editor requires a Wayland session.");
        std::process::exit(1);
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    mundane_ui::run(move |cx| {
        mundane_ui::bind_text_keys(cx);
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
                    let mut editor = Editor::new(window, cx);
                    if let Some(path) = path {
                        editor.open(path, window, cx);
                    }
                    editor
                })
            },
        ) {
            eprintln!("Could not open Text Editor. {e}");
            std::process::exit(1);
        }
    });
}
