//! Menu contents and focus management. The caller owns popup placement and dismissal at the seat.
use super::*;
#[derive(Clone)]
pub struct MenuChoice {
    pub key: SharedString,
    pub label: SharedString,
    pub enabled: bool,
}
impl MenuChoice {
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            enabled: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEvent {
    Closed,
    Selected(SharedString),
}
pub struct ChoiceMenu {
    name: SharedString,
    trigger: FocusHandle,
    choices: Vec<(MenuChoice, FocusHandle)>,
    search: Option<Entity<TextField>>,
    query: String,
    active: Option<usize>,
    open: bool,
    scroll: ScrollHandle,
}
impl EventEmitter<MenuEvent> for ChoiceMenu {}
impl ChoiceMenu {
    pub fn new(
        name: impl Into<SharedString>,
        choices: Vec<MenuChoice>,
        searchable: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = searchable.then(|| {
            let field = cx.new(|cx| TextField::new("Filter options", Theme::current(cx), cx));
            cx.observe(&field, |m, f, cx| {
                let query = f.read(cx).text().to_lowercase();
                if query != m.query {
                    m.query = query;
                    m.active = m.enabled().first().copied();
                    cx.notify();
                }
            })
            .detach();
            field
        });
        Self {
            name: name.into(),
            trigger: cx.focus_handle().tab_stop(true),
            choices: choices
                .into_iter()
                .map(|c| (c, cx.focus_handle()))
                .collect(),
            search,
            query: String::new(),
            active: None,
            open: false,
            scroll: ScrollHandle::new(),
        }
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn trigger_focus(&self) -> FocusHandle {
        self.trigger.clone()
    }
    pub fn replace(&mut self, choices: Vec<MenuChoice>, w: &mut Window, cx: &mut Context<Self>) {
        let previous = self.active.map(|i| self.choices[i].0.key.clone());
        let focus_was_in_choices = self.choices.iter().any(|(_, h)| h.is_focused(w));
        self.choices = choices
            .into_iter()
            .map(|c| (c, cx.focus_handle()))
            .collect();
        let enabled = self.enabled();
        self.active = enabled
            .iter()
            .copied()
            .find(|i| Some(&self.choices[*i].0.key) == previous.as_ref())
            .or_else(|| enabled.first().copied());
        if self.open && focus_was_in_choices {
            self.focus_active(w, cx);
        }
        cx.notify()
    }
    fn matches(&self, c: &MenuChoice) -> bool {
        self.query.is_empty() || c.label.to_lowercase().contains(&self.query)
    }
    fn enabled(&self) -> Vec<usize> {
        self.choices
            .iter()
            .enumerate()
            .filter(|(_, (c, _))| c.enabled && self.matches(c))
            .map(|(i, _)| i)
            .collect()
    }
    pub fn toggle(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.dismiss(w, cx)
        } else {
            self.show(w, cx)
        }
    }
    pub fn show(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.query.clear();
        self.active = self.enabled().first().copied();
        if let Some(field) = &self.search {
            field.update(cx, |f, cx| {
                f.select_match(0..f.text().len(), cx);
                f.replace_text_in_range(None, "", w, cx);
            });
            field.read(cx).focus_handle(cx).focus(w, cx);
        } else {
            self.focus_active(w, cx);
        }
        cx.notify()
    }
    pub fn dismiss(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            self.trigger.focus(w, cx);
            cx.emit(MenuEvent::Closed);
            cx.notify();
        }
    }
    fn focus_active(&self, w: &mut Window, cx: &mut App) {
        if let Some(i) = self.active {
            self.choices[i].1.focus(w, cx);
            let visible = self.choices[..i]
                .iter()
                .filter(|(c, _)| self.matches(c))
                .count();
            self.scroll.scroll_to_item(visible);
        } else if let Some(f) = &self.search {
            f.read(cx).focus_handle(cx).focus(w, cx);
        } else {
            self.trigger.focus(w, cx);
        }
    }
    fn choose(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        if self.open
            && let Some((c, _)) = self.choices.get(i)
            && c.enabled
            && self.matches(c)
        {
            let key = c.key.clone();
            self.dismiss(w, cx);
            cx.emit(MenuEvent::Selected(key));
        }
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let filtered = self.enabled();
        match e.keystroke.key.as_str() {
            "escape" => self.dismiss(w, cx),
            "enter" => {
                if let Some(i) = self.active {
                    self.choose(i, w, cx);
                }
            }
            dir @ ("up" | "down" | "home" | "end") if !filtered.is_empty() => {
                let current = self
                    .active
                    .and_then(|i| filtered.iter().position(|pos| *pos == i))
                    .unwrap_or(0);
                let n = filtered.len();
                let i = match dir {
                    "home" => 0,
                    "end" => n - 1,
                    "up" => (current + n - 1) % n,
                    _ if self
                        .search
                        .as_ref()
                        .is_some_and(|s| s.read(cx).focus_handle(cx).is_focused(w)) =>
                    {
                        current
                    }
                    _ => (current + 1) % n,
                };
                self.active = Some(filtered[i]);
                self.focus_active(w, cx);
            }
            "tab" => {
                if self.search.is_some() {
                    if self.choices.iter().any(|(_, h)| h.is_focused(w)) {
                        self.search
                            .as_ref()
                            .unwrap()
                            .read(cx)
                            .focus_handle(cx)
                            .focus(w, cx);
                    } else {
                        self.focus_active(w, cx);
                    }
                } else {
                    self.dismiss(w, cx);
                }
            }
            _ => return,
        }
        cx.notify();
        w.prevent_default();
        cx.stop_propagation();
    }
}
pub fn menu_trigger(
    label: &'static str,
    menu: &Entity<ChoiceMenu>,
    theme: Theme,
    cx: &App,
) -> Stateful<Div> {
    let state = menu.read(cx);
    let open = state.open;
    let focus = state.trigger.clone();
    let keys = menu.clone();
    let menu = menu.clone();
    button(label, &focus, theme, move |w, cx| {
        menu.update(cx, |m, cx| m.toggle(w, cx))
    })
    .aria_expanded(open)
    .on_key_down(move |ev, w, cx| {
        if ev.keystroke.key == "escape" && keys.read(cx).is_open() {
            keys.update(cx, |m, cx| m.dismiss(w, cx));
            cx.stop_propagation();
        }
    })
}
impl Render for ChoiceMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        div().when(self.open, |el| {
            let searchable = self.search.is_some();
            let role = if searchable {
                Role::ListBox
            } else {
                Role::Menu
            };
            let mut visible = 0;
            let choices = role
                .control("choices", self.name.clone())
                .max_h(rems(14.))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .flex()
                .flex_col()
                .children(
                    self.choices
                        .iter()
                        .enumerate()
                        .filter(|(_, (c, _))| self.matches(c))
                        .map(|(i, (c, f))| {
                            visible += 1;
                            let selected = self.active == Some(i);
                            let view = cx.weak_entity();
                            let role = if searchable {
                                Role::ListBoxOption
                            } else {
                                Role::MenuItem
                            };
                            let row = focus_ring(role.control(f, c.label.clone()), f, t)
                                .when(searchable, |el| el.aria_selected(selected))
                                .flex_none()
                                .px_2()
                                .py_1()
                                .when(selected, |el| el.bg(rgb(t.selected())))
                                .text_color(rgb(if c.enabled { t.text } else { t.disabled() }))
                                .child(c.label.clone())
                                .when(c.enabled, |el| {
                                    el.cursor_pointer().on_click(move |_, w, cx| {
                                        let _ = view.update(cx, |m, cx| m.choose(i, w, cx));
                                    })
                                });
                            unavailable(row, !c.enabled)
                        }),
                );
            el.flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded(tokens::radius::DIALOG)
                .bg(rgb(t.panel))
                .border_1()
                .border_color(rgb(t.border))
                .capture_key_down(cx.listener(Self::keys))
                .children(self.search.clone())
                .child(choices)
                .when(visible == 0, |el| {
                    el.child(
                        Role::Status
                            .control("empty", "No matching options")
                            .child("No matching options"),
                    )
                })
        })
    }
}
