use super::*;

pub(super) struct ShellTab {
    pub id: u64,
    pub session: Session,
    pub profile: profiles::Profile,
    shortcuts: std::collections::BTreeMap<&'static str, String>,
    pub(super) zoom: f32,
    pub label: String,
    pub focus: FocusHandle,
    events: async_channel::Receiver<Event>,
    watcher: Option<Task<()>>,
}
impl ShellTab {
    pub fn apply_profile(&mut self, profile: profiles::Profile) {
        self.session
            .screen
            .lock()
            .set_options(alacritty_terminal::term::Config {
                scrolling_history: profile.history,
                ..Default::default()
            });
        self.zoom = profile.size.into();
        self.shortcuts = profiles::bindings(&profile.shortcuts).expect("validated profile");
        self.profile = profile;
    }
}
impl Terminal {
    pub(super) fn start(
        id: u64,
        profiles: &profiles::Profiles,
        mut options: Options,
        layout: Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<ShellTab> {
        let profile = profiles.selected();
        profile.configure(&mut options);
        let (session, events) = Session::open(options, layout.size())?;
        let mut tab = ShellTab {
            id,
            shortcuts: Default::default(),
            profile: profile.clone(),
            zoom: layout.font_size,
            session,
            label: format!("Shell {id}"),
            focus: cx.focus_handle().tab_stop(true),
            events,
            watcher: None,
        };
        tab.apply_profile(profile.clone());
        tab.watch(window, cx);
        Ok(tab)
    }
}
impl ShellTab {
    pub(super) fn watch(&mut self, window: &mut Window, cx: &mut Context<Terminal>) {
        self.watcher.take();
        let events = self.events.clone();
        let id = self.id;
        self.watcher = Some(cx.spawn_in(window, async move |view, cx| {
            while let Ok(first) = events.recv().await {
                let result = view.update_in(cx, |terminal, window, cx| {
                    let Some(i) = terminal.tabs.iter().position(|tab| tab.id == id) else {
                        return;
                    };
                    for event in std::iter::once(first)
                        .chain(std::iter::from_fn(|| events.try_recv().ok()).take(255))
                    {
                        let tab = &mut terminal.tabs[i];
                        match event {
                            Event::Exit => {
                                terminal.close_tab(i, window, cx);
                                return;
                            }
                            Event::Title(t) => {
                                tab.label = if t.trim().is_empty() {
                                    format!("Shell {id}")
                                } else {
                                    t.chars().filter(|c| !c.is_control()).take(120).collect()
                                }
                            }
                            Event::ResetTitle => tab.label = format!("Shell {id}"),
                            Event::PtyWrite(s) => tab.session.write(s.into_bytes()),
                            Event::ColorRequest(index, reply) => {
                                let screen = tab.session.screen.lock();
                                let [_, r, g, b] = grid::palette_color(
                                    index,
                                    tab.profile.theme(Theme::current(cx)),
                                    screen.colors(),
                                )
                                .to_be_bytes();
                                tab.session.write(
                                    reply(alacritty_terminal::vte::ansi::Rgb { r, g, b })
                                        .into_bytes(),
                                );
                            }
                            Event::TextAreaSizeRequest(reply) => {
                                tab.session.write(reply(tab.session.size()).into_bytes())
                            }
                            _ => {}
                        }
                    }
                    cx.notify();
                });
                if result.is_err() {
                    break;
                }
            }
        }));
    }
}
impl Terminal {
    pub(super) fn select_tab(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        self.active = i;
        self.layout.font_size = self.tabs[i].zoom;
        self.composing.clear();
        self.wheel_remainder = 0.;
        self.find = None;
        self.notice = None;
        self.tab_scroll.scroll_to_item(i);
        self.grid_focus.focus(w, cx);
        cx.notify();
    }
    pub(super) fn add_tab(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let mut opts = self.options.clone();
        if let Some(dir) = self.tabs[self.active].session.working_directory() {
            opts.working_directory = Some(dir);
        }
        match Self::start(self.next_id, &self.profiles, opts, self.layout, w, cx) {
            Ok(tab) => {
                self.next_id += 1;
                self.tabs.push(tab);
                self.select_tab(self.tabs.len() - 1, w, cx);
            }
            Err(e) => {
                self.notice = Some(format!("Could not open a shell. {e}"));
                cx.notify();
            }
        }
    }
    pub(super) fn close_tab(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() == 1 {
            w.remove_window();
            return;
        }
        self.tabs.remove(i);
        if i == self.active {
            self.select_tab(i.min(self.tabs.len() - 1), w, cx)
        } else {
            if i < self.active {
                self.active -= 1
            }
            self.tab_scroll.scroll_to_item(self.active);
            cx.notify()
        }
    }
    pub(super) fn tab_key(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        if self.warning.is_some() || self.form.is_some() {
            let handled = if self.warning.is_some() {
                match k.key.as_str() {
                    "escape" => {
                        self.answer_close(false, w, cx);
                        true
                    }
                    "tab" => {
                        let warn = self.warning.as_ref().unwrap();
                        warn.buttons[usize::from(warn.buttons[0].is_focused(w))].focus(w, cx);
                        true
                    }
                    _ => false,
                }
            } else {
                self.form_key(k, w, cx)
            };
            if handled {
                w.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        if self.find.is_some() {
            return;
        }
        let k = &e.keystroke;
        let key = k.unparse(OperatingSystem::Linux);
        let action = self.tabs[self.active]
            .shortcuts
            .iter()
            .find(|(_, s)| **s == key)
            .map(|(a, _)| *a);
        match action {
            Some("tab") => self.add_tab(w, cx),
            Some("window") => self.separate(false, w, cx),
            Some("detach") => self.separate(true, w, cx),
            Some("close") => self.request_close(Target::Tab(self.tabs[self.active].id), w, cx),
            Some("profiles") => self.edit_profiles(w, cx),
            _ => match (k.key.as_str(), k.modifiers.control, k.modifiers.shift) {
                ("pageup" | "pagedown", true, false) => {
                    let n = self.tabs.len();
                    let delta = if k.key == "pageup" { n - 1 } else { 1 };
                    self.select_tab((self.active + delta) % n, w, cx)
                }
                _ => return,
            },
        }
        w.prevent_default();
        cx.stop_propagation();
    }
    pub(super) fn strip(&self, cx: &Context<Self>) -> Div {
        let view = cx.weak_entity();
        let close = view.clone();
        let t = Theme::current(cx);
        mundane_ui::tab_strip(
            self.tabs
                .iter()
                .map(|tab| (tab.label.clone().into(), tab.focus.clone())),
            self.active,
            &self.tab_scroll,
            move |i, w, cx| {
                let _ = view.update(cx, |v, cx| v.select_tab(i, w, cx));
            },
            div()
                .id("tab-actions")
                .flex()
                .gap_1()
                .child(button("Detach", &self.detach_focus, t, {
                    let v = cx.weak_entity();
                    move |w, cx| {
                        let _ = v.update(cx, |s, cx| s.separate(true, w, cx));
                    }
                }))
                .child(button("Close tab", &self.close_focus, t, move |w, cx| {
                    let _ = close.update(cx, |v, cx| {
                        v.request_close(Target::Tab(v.tabs[v.active].id), w, cx)
                    });
                })),
            t,
        )
    }
}
