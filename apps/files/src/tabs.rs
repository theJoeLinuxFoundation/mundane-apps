use super::*;

/// Re-read the directory on return so a move or rename in another tab is visible.
pub(super) struct FolderTab {
    path: PathBuf,
    selected: Option<PathBuf>,
    query: Option<String>,
    marks: selection::Selection,
    hidden: bool,
    focus: FocusHandle,
}
impl FolderTab {
    pub fn new(path: PathBuf, cx: &App) -> Self {
        Self {
            path,
            selected: None,
            query: None,
            marks: Default::default(),
            hidden: false,
            focus: cx.focus_handle().tab_stop(true),
        }
    }
}
impl Files {
    fn remember_tab(&mut self) {
        let selected = self.selected().map(|e| e.path.clone());
        let tab = &mut self.tabs[self.active];
        tab.path = self.requested.clone().unwrap_or_else(|| self.path.clone());
        tab.selected = selected;
        tab.marks = self.marks.clone();
        tab.query = self.nav.query.clone();
        tab.hidden = self.show_hidden;
    }
    fn display_tab(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        self.active = i;
        let tab = &self.tabs[i];
        self.path = tab.path.clone();
        self.marks = tab.marks.clone();
        self.show_hidden = tab.hidden;
        let path = tab.path.clone();
        let selected = tab.selected.clone();
        let query = tab.query.clone();
        self.entries.clear();
        self.selected = None;
        self.scroll = UniformListScrollHandle::new();
        self.select_place();
        if let Some(q) = query {
            self.search_with_selection(q, selected, cx);
        } else {
            self.load(path, selected, cx);
        }
        self.tab_scroll.scroll_to_item(i);
        self.list_focus.focus(w, cx);
        cx.notify();
    }
    pub(super) fn switch_tab(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        if self.working || self.dialog.is_some() || i == self.active || i >= self.tabs.len() {
            return;
        }
        self.remember_tab();
        self.display_tab(i, w, cx)
    }
    pub(super) fn new_tab(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.working || self.dialog.is_some() {
            return;
        }
        self.remember_tab();
        let current = &self.tabs[self.active];
        let mut tab = FolderTab::new(current.path.clone(), cx);
        tab.hidden = current.hidden;
        tab.selected = current.selected.clone();
        self.tabs.push(tab);
        self.display_tab(self.tabs.len() - 1, w, cx);
    }
    pub(super) fn close_tab(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.working || self.dialog.is_some() {
            return;
        }
        if self.tabs.len() == 1 {
            w.remove_window();
            return;
        }
        self.tabs.remove(self.active);
        self.display_tab(self.active.min(self.tabs.len() - 1), w, cx);
    }
    pub(super) fn tab_keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() || !e.keystroke.modifiers.control {
            return;
        }
        match e.keystroke.key.as_str() {
            "t" => self.new_tab(w, cx),
            "w" => self.close_tab(w, cx),
            "pageup" | "pagedown" => {
                let n = self.tabs.len();
                let step = if e.keystroke.key == "pageup" {
                    n - 1
                } else {
                    1
                };
                self.switch_tab((self.active + step) % n, w, cx);
            }
            _ => return,
        }
        w.prevent_default();
        cx.stop_propagation();
    }
    pub(super) fn tab_strip(&self, cx: &Context<Self>) -> Div {
        let v = cx.weak_entity();
        let close = v.clone();
        let theme = Theme::current(cx);
        mundane_ui::tab_strip(
            self.tabs.iter().enumerate().map(|(i, t)| {
                let path = if i == self.active {
                    &self.path
                } else {
                    &t.path
                };
                let name = path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
                    .into_owned();
                (name.into(), t.focus.clone())
            }),
            self.active,
            &self.tab_scroll,
            move |i, w, cx| {
                let _ = v.update(cx, |s, cx| s.switch_tab(i, w, cx));
            },
            button("Close tab", &self.close_focus, theme, move |w, cx| {
                let _ = close.update(cx, |s, cx| s.close_tab(w, cx));
            }),
            theme,
        )
    }
}
