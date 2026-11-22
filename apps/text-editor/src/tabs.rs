use super::*;

pub(super) struct Page {
    pub document: Document,
    pub body: Entity<TextField>,
    focus: FocusHandle,
    untitled: usize,
}

impl Page {
    pub(super) fn new(untitled: usize, cx: &mut Context<Editor>) -> Self {
        let theme = Theme::current(cx);
        let body =
            cx.new(|cx| TextField::multiline("Document text", theme, cx).with_line_numbers());
        cx.observe(&body, |_, _, cx| cx.notify()).detach();
        Self {
            body,
            document: Document::default(),
            focus: cx.focus_handle().tab_stop(true),
            untitled,
        }
    }

    pub fn dirty(&self, cx: &App) -> bool {
        self.body.read(cx).text() != self.document.saved.as_str()
    }

    pub fn name(&self) -> String {
        self.document
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| match self.untitled {
                1 => "Untitled".into(),
                n => format!("Untitled {n}"),
            })
    }
}

impl Editor {
    pub(super) fn page(&self) -> &Page {
        &self.pages[self.active]
    }

    pub(super) fn switch(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy.is_some() || self.confirmation.is_some() {
            return;
        }
        self.active = i;
        self.scroll.scroll_to_item(i);
        self.searching = false;
        self.going = false;
        self.error = None;
        self.focus_body(w, cx);
        cx.notify();
    }

    pub(super) fn new_tab(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.pages.push(Page::new(self.next_id, cx));
        self.next_id += 1;
        self.switch(self.pages.len() - 1, w, cx);
    }

    pub(super) fn strip(&self, cx: &Context<Self>) -> Div {
        let view = cx.weak_entity();
        mundane_ui::tab_strip(
            self.pages.iter().map(|p| {
                let mut name = p.name();
                if p.dirty(cx) {
                    name.push_str(" •");
                }
                (name.into(), p.focus.clone())
            }),
            self.active,
            &self.scroll,
            move |i, w, cx| {
                let _ = view.update(cx, |e, cx| e.switch(i, w, cx));
            },
            self.tool(Command::CloseTab, &self.close_focus, cx),
            Theme::current(cx),
        )
    }
}
