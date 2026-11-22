//! Passive notifications and level feedback. Owners supply stable keys and handle all commands.
use super::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub struct Notice {
    pub key: SharedString,
    pub title: SharedString,
    pub body: SharedString,
    pub source: Option<SharedString>,
    pub actions: Vec<(SharedString, SharedString)>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum NoticeAction {
    Dismiss,
    Invoke(SharedString),
}
type SendAction = Rc<dyn Fn(SharedString, NoticeAction, &mut Window, &mut App)>;

fn live(control: Stateful<Div>) -> Stateful<Div> {
    control.a11y_synthetic_children(|tree| {
        let node = tree.parent_node();
        node.set_live(accesskit::Live::Polite);
        node.set_live_atomic();
    })
}
pub struct NotificationCard {
    notice: Notice,
    focus: FocusHandle,
    dismiss: FocusHandle,
    actions: Vec<(SharedString, FocusHandle)>,
}
impl NotificationCard {
    pub fn new(n: Notice, cx: &App) -> Self {
        let actions = n
            .actions
            .iter()
            .map(|(key, _)| (key.clone(), cx.focus_handle().tab_stop(true)))
            .collect();
        Self {
            notice: n,
            focus: cx.focus_handle(),
            dismiss: cx.focus_handle().tab_stop(true),
            actions,
        }
    }
    pub fn update(&mut self, n: Notice, cx: &App) {
        self.actions = n
            .actions
            .iter()
            .map(|(key, _)| {
                let f = self
                    .actions
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, f)| f.clone())
                    .unwrap_or_else(|| cx.focus_handle().tab_stop(true));
                (key.clone(), f)
            })
            .collect();
        self.notice = n;
    }
    pub fn render(
        &self,
        t: Theme,
        send: impl Fn(SharedString, NoticeAction, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        self.card(t, true, Rc::new(send))
    }
    fn card(&self, t: Theme, announce: bool, send: SendAction) -> Stateful<Div> {
        let n = &self.notice;
        let key = n.key.clone();
        let dismiss = send.clone();
        let id = key.clone();
        let summary = format!("{}. {}", n.title, n.body);
        let body = if announce {
            live(Role::Status.control("body", summary))
        } else {
            Role::Label.control("body", n.body.clone())
        };
        Role::Article
            .control(&self.focus, n.title.clone())
            .track_focus(&self.focus)
            .border_1()
            .border_color(rgb(t.border))
            .focus(move |s| s.border_color(rgb(t.focus)))
            .p(tokens::space::MD)
            .rounded(tokens::radius::DIALOG)
            .bg(rgb(t.panel))
            .flex()
            .flex_col()
            .gap_1()
            .when_some(n.source.clone(), |el, source| {
                el.child(
                    div()
                        .text_size(tokens::text::CAPTION)
                        .text_color(rgb(t.muted()))
                        .child(source),
                )
            })
            .child(
                Role::Heading
                    .control("heading", n.title.clone())
                    .aria_level(2)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(n.title.clone()),
            )
            .child(body.child(n.body.clone()))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .flex_wrap()
                    .children(n.actions.iter().zip(&self.actions).map(
                        |((action, label), (_, focus))| {
                            let send = send.clone();
                            let key = key.clone();
                            let action = action.clone();
                            button_base(label.clone(), focus, t, move |w, cx| {
                                send(key.clone(), NoticeAction::Invoke(action.clone()), w, cx)
                            })
                            .px_2()
                            .py_1()
                            .child(label.clone())
                        },
                    ))
                    .child(button("Dismiss", &self.dismiss, t, move |w, cx| {
                        dismiss(id.clone(), NoticeAction::Dismiss, w, cx)
                    })),
            )
    }
}

pub struct NotificationHistory {
    cards: Vec<NotificationCard>,
    active: Rc<Cell<usize>>,
    focus: FocusHandle,
    scroll: ScrollHandle,
}
impl NotificationHistory {
    pub fn new(cx: &App) -> Self {
        Self {
            cards: vec![],
            active: Default::default(),
            focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
        }
    }
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }
    pub fn selected_key(&self) -> Option<&SharedString> {
        self.cards.get(self.active.get()).map(|c| &c.notice.key)
    }
    pub fn replace(&mut self, notices: Vec<Notice>, w: &mut Window, cx: &mut App) {
        let focused = self
            .cards
            .iter()
            .find(|c| c.focus.contains_focused(w, cx))
            .map(|c| c.notice.key.clone());
        let selected = focused.clone().or_else(|| self.selected_key().cloned());
        let mut old = std::mem::take(&mut self.cards)
            .into_iter()
            .map(|card| (card.notice.key.clone(), card))
            .collect::<std::collections::HashMap<_, _>>();
        self.cards = notices
            .into_iter()
            .map(|n| match old.remove(&n.key) {
                Some(mut c) => {
                    c.update(n, cx);
                    c
                }
                None => NotificationCard::new(n, cx),
            })
            .collect();
        self.active.set(
            selected
                .and_then(|key| self.cards.iter().position(|c| c.notice.key == key))
                .unwrap_or(self.active.get().min(self.cards.len().saturating_sub(1))),
        );
        if let Some(key) = focused {
            if old.contains_key(&key) {
                self.focus_active(w, cx);
            } else {
                self.scroll.scroll_to_item(self.active.get());
            }
        }
    }
    pub fn remove(&mut self, key: &SharedString, w: &mut Window, cx: &mut App) {
        if let Some(i) = self.cards.iter().position(|c| &c.notice.key == key) {
            let focused = self.cards[i].focus.contains_focused(w, cx);
            self.cards.remove(i);
            if focused {
                self.active.set(i);
            } else if i < self.active.get() {
                self.active.set(self.active.get() - 1);
            }
            self.active
                .set(self.active.get().min(self.cards.len().saturating_sub(1)));
            if focused {
                self.focus_active(w, cx);
            }
        }
    }
    fn focus_active(&self, w: &mut Window, cx: &mut App) {
        if let Some(c) = self.cards.get(self.active.get()) {
            c.focus.focus(w, cx);
            self.scroll.scroll_to_item(self.active.get());
        } else {
            self.focus.focus(w, cx);
        }
    }
    pub fn render(
        &self,
        name: impl Into<SharedString>,
        t: Theme,
        send: impl Fn(SharedString, NoticeAction, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let send: SendAction = Rc::new(send);
        let focus = self.focus.clone();
        let active = self.active.clone();
        let cards: Vec<_> = self.cards.iter().map(|card| card.focus.clone()).collect();
        let scroll = self.scroll.clone();
        focus_ring(
            Role::List
                .control(&self.focus, name)
                .aria_size_of_set(self.cards.len()),
            &self.focus,
            t,
        )
        .flex()
        .flex_col()
        .gap_2()
        .overflow_y_scroll()
        .track_scroll(&self.scroll)
        .on_key_down(move |event, w, cx| {
            if cards.is_empty() {
                return;
            }
            let current = cards
                .iter()
                .position(|card| card.contains_focused(w, cx))
                .unwrap_or(active.get());
            let last = cards.len() - 1;
            let next = match event.keystroke.key.as_str() {
                "up" => current.saturating_sub(1),
                "down" if focus.is_focused(w) => current,
                "down" => (current + 1).min(last),
                "home" => 0,
                "end" => last,
                _ => return,
            };
            active.set(next);
            cards[next].focus(w, cx);
            scroll.scroll_to_item(next);
            w.prevent_default();
            cx.stop_propagation();
        })
        .min_h_0()
        .when(self.cards.is_empty(), |el| {
            el.child(
                Role::Status
                    .control("empty", "No notifications")
                    .child("No notifications"),
            )
        })
        .children(self.cards.iter().enumerate().map(|(i, c)| {
            Role::ListItem
                .control(c.notice.key.clone(), c.notice.title.clone())
                .aria_position_in_set(i)
                .flex_none()
                .child(c.card(t, false, send.clone()))
        }))
    }
}

/// A read-only meter and a live text equivalent. No hit action, focus handle or keyboard grab.
pub fn level_osd(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    percent: u8,
    muted: bool,
    t: Theme,
) -> Stateful<Div> {
    let label = label.into();
    let level = percent.min(100);
    let value = if muted {
        format!("Muted · {level}%")
    } else {
        format!("{level}%")
    };
    let announcement = format!("{label}: {value}");
    Role::Meter
        .control(id, label.clone())
        .aria_min_numeric_value(0.)
        .aria_max_numeric_value(100.)
        .aria_numeric_value(level.into())
        .aria_value(value.clone())
        .p(tokens::space::MD)
        .rounded(tokens::radius::DIALOG)
        .border_1()
        .border_color(rgb(t.border))
        .bg(rgb(t.panel))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            live(Role::Status.control("level", announcement))
                .flex()
                .justify_between()
                .child(label)
                .child(value),
        )
        .child(
            div().w_full().h(px(5.)).bg(rgb(t.border)).child(
                div()
                    .w(relative(f32::from(level) / 100.))
                    .h_full()
                    .bg(rgb(if muted { t.disabled() } else { t.focus })),
            ),
        )
}
