use super::*;

pub(super) fn options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(820.), px(520.)), cx);
    WindowOptions {
        app_id: Some(APP_ID.into()),
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    }
}
impl Terminal {
    pub(super) fn separate(&mut self, detach: bool, w: &mut Window, cx: &mut Context<Self>) {
        if detach && self.tabs.len() < 2 {
            return;
        }
        let mut opts = self.options.clone();
        if let Some(dir) = self.tabs[self.active].session.working_directory() {
            opts.working_directory = Some(dir);
        }
        // Window creation can fail before GPUI invokes the view constructor.
        // Keep a detached tab in its original window until the new surface exists.
        let mut fresh = None;
        if !detach {
            match Self::start(1, &self.profiles, opts.clone(), self.layout, w, cx) {
                Ok(tab) => fresh = Some(tab),
                Err(e) => {
                    self.notice = Some(format!("Could not open a shell. {e}"));
                    cx.notify();
                    return;
                }
            }
        }
        let window_options = options(cx);
        let result = cx.open_window(window_options, |next, cx| {
            next.set_window_title("Terminal");
            let mut tab = fresh.unwrap_or_else(|| self.tabs.remove(self.active));
            let profiles = self.profiles.clone();
            cx.new(|cx| {
                tab.watch(next, cx);
                let layout = Layout {
                    bounds: Bounds::new(Point::default(), next.viewport_size()),
                    font_size: tab.zoom,
                };
                Terminal::with_tab(opts, profiles, None, layout, tab, next, cx)
            })
        });
        match result {
            Ok(_) if detach => self.select_tab(self.active.min(self.tabs.len() - 1), w, cx),
            Err(e) => self.notice = Some(format!("Could not open a window. {e}")),
            _ => {}
        }
        cx.notify()
    }
}
