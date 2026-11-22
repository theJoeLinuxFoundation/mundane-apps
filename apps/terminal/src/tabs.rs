use super::*;

pub(super) struct ShellTab {
    pub id: u64,
    pub session: Session,
    pub profile: profiles::Profile,
    pub(super) zoom: f32,
    pub label: String,
    pub focus: FocusHandle,
    _events: Task<()>,
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
        self.profile = profile;
    }
}
impl Terminal {
    pub(super) fn shell(
        id: u64,
        options: Options,
        layout: Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<ShellTab> {
        let (session, events) = Session::open(options, layout.size())?;
        let task = cx.spawn_in(window, async move |view, cx| {
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
        });
        Ok(ShellTab {
            id,
            profile: Default::default(),
            zoom: BODY_FONT,
            session,
            label: format!("Shell {id}"),
            focus: cx.focus_handle().tab_stop(true),
            _events: task,
        })
    }
    fn select_tab(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
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
        match Self::shell(self.next_id, opts, self.layout, w, cx) {
            Ok(mut tab) => {
                tab.apply_profile(self.profiles.selected().clone());
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
        match (k.key.as_str(), k.modifiers.control, k.modifiers.shift) {
            ("t", true, true) => self.add_tab(w, cx),
            ("w", true, true) => self.request_close(Target::Tab(self.tabs[self.active].id), w, cx),
            ("p", true, true) => self.edit_profiles(w, cx),
            ("pageup" | "pagedown", true, false) => {
                let n = self.tabs.len();
                let delta = if k.key == "pageup" { n - 1 } else { 1 };
                self.select_tab((self.active + delta) % n, w, cx)
            }
            _ => return,
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
            button("Close tab", &self.close_focus, t, move |w, cx| {
                let _ = close.update(cx, |v, cx| {
                    v.request_close(Target::Tab(v.tabs[v.active].id), w, cx)
                });
            }),
            t,
        )
    }
}
