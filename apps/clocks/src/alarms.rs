use super::*;
use mundane_services::clocks::alarms::{self, Action, Alarm};

pub(super) struct Alarms {
    name: Entity<TextField>,
    pub(super) time: Entity<TextField>,
    mask: u8,
    days: [FocusHandle; 7],
    buttons: [FocusHandle; 4],
    records: Vec<Alarm>,
    checked: i64,
}
impl Alarms {
    pub(super) fn new(cx: &mut Context<Clocks>) -> Self {
        let t = Theme::current(cx);
        Self {
            name: cx.new(|cx| TextField::new("Alarm name", t, cx)),
            time: cx.new(|cx| {
                TextField::new("Alarm time (24-hour HH:MM)", t, cx).select_value("07:00".into())
            }),
            days: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            buttons: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            mask: 0,
            records: vec![],
            checked: 0,
        }
    }
}
impl Clocks {
    fn alarm_path(&self) -> anyhow::Result<PathBuf> {
        self.path
            .as_ref()
            .map(|p| p.with_file_name("alarms.json"))
            .ok_or_else(|| anyhow::anyhow!("No settings directory"))
    }
    pub(super) fn alarm_rows(&mut self) -> Vec<BrowserRow> {
        self.alarms.records = match self.alarm_path().and_then(|p| alarms::read(&p)) {
            Ok(a) => a,
            Err(e) => {
                self.notice = e.to_string();
                vec![]
            }
        };
        self.alarms
            .records
            .iter()
            .map(|a| BrowserRow {
                key: a.id.to_string().into(),
                label: a.name.clone(),
                detail: a.repeat(),
                value: if a.ringing {
                    "Ringing".into()
                } else if a.snoozed.is_some() {
                    "Snoozed".into()
                } else if a.next.is_none() {
                    "Finished".into()
                } else {
                    a.time()
                },
                depth: 0,
                fraction: None,
            })
            .collect()
    }
    pub(super) fn alarm_tick(&mut self, cx: &mut Context<Self>) {
        let now = self.fixed.unwrap_or_else(clocks::wall_now);
        if self.page == Page::Alarms && now != self.alarms.checked {
            self.alarms.checked = now;
            self.refresh(cx);
        }
    }
    pub(super) fn alarms_focus(&self) -> FocusHandle {
        if self.list.rows.is_empty() {
            self.alarms.buttons[0].clone()
        } else {
            self.list.focus.clone()
        }
    }
    pub(super) fn alarm_focus(&self, cx: &App) -> FocusHandle {
        self.alarms.name.focus_handle(cx)
    }
    fn selected_alarm(&self) -> Option<&Alarm> {
        self.alarms.records.get(self.list.selected?)
    }
    pub(super) fn update_alarm(
        &mut self,
        action: Option<Action>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = self.fixed.unwrap_or_else(clocks::wall_now);
        let result = self.alarm_path().and_then(|path| {
            if let Some(action) = action {
                let Some(a) = self.selected_alarm() else {
                    return Ok(());
                };
                alarms::change(&path, a.id, action, now)?;
            } else {
                alarms::add(
                    &path,
                    self.alarms.name.read(cx).text(),
                    self.alarms.time.read(cx).text(),
                    self.alarms.mask,
                    now,
                    &clocks::LocalZone::system(),
                )?;
            }
            if self.fixed.is_none() {
                std::process::Command::new(std::env::current_exe()?)
                    .arg("--watch-alarms")
                    .stdin(std::process::Stdio::null())
                    .spawn()
                    .map(|mut child| {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    })?;
            }
            Ok(())
        });
        match result {
            Ok(()) => self.page(Page::Alarms, w, cx),
            Err(e) => {
                self.notice = e.to_string();
                cx.notify();
            }
        }
    }
    pub(super) fn alarm_panel(&mut self, w: &mut Window, cx: &mut Context<Self>) -> Div {
        let t = Theme::current(cx);
        let adding = self.page == Page::NewAlarm;
        let mut actions = div().flex().flex_wrap().gap_2();
        let ringing = self.selected_alarm().is_some_and(|a| a.ringing);
        for (i, (label, act, enabled)) in [
            (if adding { "Save alarm" } else { "Add alarm" }, None, true),
            ("Snooze 9 min", Some(Action::Snooze), ringing),
            ("Dismiss", Some(Action::Dismiss), ringing),
            (
                "Remove",
                Some(Action::Remove),
                self.selected_alarm().is_some(),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            if (adding && i > 0) || (!ringing && (i == 1 || i == 2)) {
                continue;
            }
            let v = cx.weak_entity();
            actions = actions.child(mundane_ui::unavailable(
                button(label, &self.alarms.buttons[i], t, move |w, cx| {
                    if enabled {
                        let _ = v.update(cx, |v, cx| {
                            if i == 0 && !adding {
                                v.new_alarm(w, cx)
                            } else {
                                v.update_alarm(act, w, cx)
                            }
                        });
                    }
                }),
                !enabled,
            ));
        }
        let mut panel = div().flex().flex_col().flex_1().min_h_0().gap_2().child(
            div()
                .p_3()
                .bg(rgb(t.panel))
                .flex()
                .justify_between()
                .items_center()
                .child(heading(
                    "alarms",
                    if adding { "New alarm" } else { "Alarms" },
                ))
                .child(actions),
        );
        if adding {
            panel = panel.child(
                div()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child("Name")
                    .child(self.alarms.name.clone())
                    .child("Local time (24-hour)")
                    .child(self.alarms.time.clone())
                    .child("Repeat days")
                    .child(div().flex().flex_wrap().gap_1().children(
                        alarms::DAYS.iter().enumerate().map(|(i, day)| {
                            let v = cx.weak_entity();
                            mundane_ui::toggle(
                                *day,
                                &self.alarms.days[i],
                                self.alarms.mask & (1 << i) != 0,
                                true,
                                t,
                                move |_, _, cx| {
                                    let _ = v.update(cx, |v, cx| {
                                        v.alarms.mask ^= 1 << i;
                                        cx.notify()
                                    });
                                },
                            )
                        }),
                    )),
            );
        } else if self.list.rows.is_empty() {
            panel = panel.child(div().p_3().child("No alarms. Choose Add alarm to set one."));
        } else {
            let v = cx.weak_entity();
            panel = panel.child(
                self.list
                    .render("Scheduled alarms", t, w, move |i, _, w, cx| {
                        let _ = v.update(cx, |v, cx| {
                            v.list.select(i);
                            v.list.focus.focus(w, cx);
                            cx.notify();
                        });
                    }),
            );
        }
        panel
    }
    pub(super) fn new_alarm(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.alarms = Alarms::new(cx);
        self.page(Page::NewAlarm, w, cx);
    }
}
