use super::*;
use mundane_ui::{ControlRole, Role, button, launcher_row, unavailable};

const REVIEW_PAGE: usize = 5;
pub(super) fn label(id: impl Into<ElementId>, text: impl Into<String>) -> Stateful<Div> {
    let text = text.into();
    mundane_ui::text_label(id, text.clone()).child(text)
}
fn size_text(bytes: u64) -> String {
    for (unit, factor) in [("GiB", 1 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)] {
        if bytes >= factor {
            return format!("{:.1} {unit}", bytes as f64 / factor as f64);
        }
    }
    format!("{bytes} B")
}
fn section(id: &'static str, name: &'static str) -> Stateful<Div> {
    mundane_ui::heading(id, name)
        .font_weight(FontWeight::SEMIBOLD)
        .text_lg()
}
impl Packages {
    pub(super) fn control(
        &self,
        name: &'static str,
        focus: &FocusHandle,
        cx: &Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let view = cx.weak_entity();
        unavailable(
            button(name, focus, Theme::current(cx), move |w, cx| {
                let _ = view.update(cx, |this, cx| {
                    if !this.busy {
                        action(this, w, cx)
                    }
                });
            }),
            self.busy,
        )
    }
    fn keys(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        if k.modifiers.control && matches!(k.key.as_str(), "+" | "=" | "-" | "0") {
            let size = match k.key.as_str() {
                "-" => (w.rem_size() - px(2.)).max(px(12.)),
                "0" => px(16.),
                _ => (w.rem_size() + px(2.)).min(px(24.)),
            };
            w.set_rem_size(size);
        } else if let Some(review) = &self.review {
            let p = review.plan();
            let count = p.install.len() + p.upgrade.len() + p.remove.len();
            match k.key.as_str() {
                "escape" => self.finish(false, w, cx),
                "pageup" => self.review_offset = self.review_offset.saturating_sub(REVIEW_PAGE),
                "pagedown" if self.review_offset + REVIEW_PAGE < count => {
                    self.review_offset += REVIEW_PAGE
                }
                "tab" => {
                    let mut order = vec![&self.cancel];
                    if p.digest.is_some() {
                        order.push(&self.confirm)
                    }
                    if self.review_offset > 0 {
                        order.push(&self.dialog_prev)
                    }
                    if self.review_offset + REVIEW_PAGE < count {
                        order.push(&self.dialog_next)
                    }
                    let pos = order.iter().position(|f| f.is_focused(w)).unwrap_or(0);
                    order[(pos
                        + if k.modifiers.shift {
                            order.len() - 1
                        } else {
                            1
                        })
                        % order.len()]
                    .focus(w, cx)
                }
                _ => return,
            }
        } else {
            match k.key.as_str() {
                "w" if k.modifiers.control => w.remove_window(),
                "d" if k.modifiers.control => self.explore(w, cx),
                "escape" if matches!(self.page, Page::Discover(_)) && !self.busy => {
                    self.catalog_back(w, cx)
                }
                "u" if k.modifiers.control => self.start(Job::Status, w, cx),
                "f" if k.modifiers.control => self.field.focus_handle(cx).focus(w, cx),
                "f5" => self.refresh(w, cx),
                "f6" => self.installed.focus(w, cx),
                "enter" if self.field.focus_handle(cx).is_focused(w) => self.search(w, cx),
                "escape" if matches!(self.page, Page::Details(_)) => self.back(w, cx),
                "tab" if k.modifiers.shift => w.focus_prev(cx),
                "tab" => w.focus_next(cx),
                "pageup" | "pagedown" => {
                    let old = self.scroll.offset();
                    self.scroll.set_offset(point(
                        old.x,
                        old.y
                            + if k.key == "pageup" {
                                px(200.)
                            } else {
                                px(-200.)
                            },
                    ));
                }
                _ => return,
            }
        }
        w.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn page(&self, cx: &Context<Self>) -> Div {
        let body = div().flex().flex_col().gap_2().p_3();
        match &self.page {
            Page::Discover(apps) => self.catalog_page(apps, body, cx),
            Page::Installed(list) => self.list_page(
                body.child(section("installed-heading", "Installed packages")),
                list.len(),
                list.iter().skip(self.offset).take(PAGE_SIZE).map(|p| {
                    (
                        p.name.clone(),
                        format!(
                            "{} · {}",
                            p.version,
                            if p.requested {
                                "Requested"
                            } else {
                                "Dependency"
                            }
                        ),
                    )
                }),
                cx,
            ),
            Page::Results(_, s) => self.list_page(
                body.child(section("search-heading", "Search results"))
                    .child(
                        label("index", s.index.label())
                            .text_color(rgb(Theme::current(cx).secondary)),
                    ),
                s.packages.len(),
                s.packages
                    .iter()
                    .skip(self.offset)
                    .take(PAGE_SIZE)
                    .map(|p| {
                        (
                            p.name.clone(),
                            format!(
                                "{} · {}{}",
                                p.version,
                                p.description,
                                p.installed
                                    .as_ref()
                                    .map(|v| format!(" · Installed {v}"))
                                    .unwrap_or_default()
                            ),
                        )
                    }),
                cx,
            ),
            Page::Details(i) => {
                let mut page = body
                    .child(self.control("Back to packages", &self.back, cx, Self::back))
                    .child(
                        label("name", i.name.clone())
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_xl(),
                    )
                    .child(label("description", i.description.clone()));
                if let Some(a) = &i.available {
                    page = page.child(label(
                        "available",
                        format!(
                            "Available {} · {} · {} download",
                            a.version,
                            a.repository,
                            size_text(a.download_size)
                        ),
                    ))
                }
                if let Some(s) = &i.installed {
                    page = page.child(label(
                        "installed",
                        format!(
                            "Installed {} · {} · {} · {} files",
                            s.version,
                            s.reason,
                            size_text(s.size),
                            s.files
                        ),
                    ));
                    if let Some(n) = &s.needed_by {
                        page = page.child(label("needed", format!("Needed by {n}")))
                    }
                }
                page.child(label("licence", format!("Licence · {}", i.license)))
                    .child(label(
                        "depends",
                        format!("Depends on · {}", names(&i.depends)),
                    ))
                    .child(label(
                        "required",
                        format!("Required by · {}", names(&i.required_by)),
                    ))
                    .child(div().flex().child(self.control(
                        if i.installed.is_some() {
                            "Review removal…"
                        } else {
                            "Review installation…"
                        },
                        &self.action,
                        cx,
                        Self::preview,
                    )))
            }
            Page::Status(s) => {
                let mut page = body
                    .child(section("updates-heading", "System updates"))
                    .child(div().flex().child(self.control(
                        "Review system upgrade…",
                        &self.action,
                        cx,
                        Self::preview,
                    )));
                if !s.managed {
                    return page.child(label("managed", "This system does not use deployments."));
                }
                page = page.child(label(
                    "running",
                    format!("Running deployment · {}", deployment(s.running)),
                ));
                if !s.complete {
                    return page.child(label("partial","Deployment status is partial. The boot service has yet to publish the full record."));
                }
                page = page
                    .child(label(
                        "next",
                        format!("Next boot · {}", deployment(s.next_boot)),
                    ))
                    .child(label(
                        "prepared",
                        s.prepared_upgrade.as_ref().map_or_else(
                            || "No upgrade prepared for shutdown.".into(),
                            |p| {
                                format!(
                                    "Prepared for shutdown · {} ({} install or upgrade, {} remove)",
                                    p.summary, p.install, p.remove
                                )
                            },
                        ),
                    ));
                if let Some(last) = &s.last_upgrade {
                    page = page.child(label(
                        "last",
                        format!(
                            "Last upgrade · {} · deployment {} · {}",
                            last.outcome.replace('_', " "),
                            deployment(last.deployment),
                            last.message
                        ),
                    ))
                }
                for d in &s.deployments {
                    page = page.child(label(
                        ("deployment", d.id as usize),
                        format!(
                            "Deployment {} · {} · {}{}",
                            d.id,
                            d.reason,
                            d.health.as_deref().unwrap_or("Health unknown"),
                            if d.pinned { " · Pinned" } else { "" }
                        ),
                    ))
                }
                page
            }
        }
    }

    fn list_page(
        &self,
        mut body: Div,
        count: usize,
        rows: impl Iterator<Item = (String, String)>,
        cx: &Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        if count == 0 && !self.busy {
            let message = if matches!(&self.page, Page::Results(_, s) if s.index == mundane_services::packages::Index::None)
            {
                "Search will be available after the first repository refresh."
            } else {
                "No packages found."
            };
            return body.child(label("empty", message));
        }
        for (i, (name, detail)) in rows.enumerate() {
            let view = cx.weak_entity();
            let target = name.clone();
            let row = launcher_row(
                format!("Inspect {name}"),
                &self.rows[i],
                false,
                !self.busy,
                theme,
                move |w, cx| {
                    let _ = view.update(cx, |v, cx| v.start(Job::Info(target.clone()), w, cx));
                },
            )
            .aria_description(detail.clone())
            .px_3()
            .py_1()
            .flex()
            .flex_col()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
            .child(div().text_sm().child(detail));
            body = body.child(row)
        }
        self.pagination(body, count, cx)
    }
    pub(super) fn pagination(&self, mut body: Div, count: usize, cx: &Context<Self>) -> Div {
        if count > PAGE_SIZE {
            let mut nav = div().flex().gap_2().items_center();
            if self.offset > 0 {
                nav =
                    nav.child(
                        self.control("Previous packages", &self.previous, cx, |v, w, cx| {
                            v.offset = v.offset.saturating_sub(PAGE_SIZE);
                            v.rows[0].focus(w, cx);
                            cx.notify()
                        }),
                    )
            }
            nav = nav.child(label(
                "range",
                format!(
                    "{}–{} of {count}",
                    self.offset + 1,
                    (self.offset + PAGE_SIZE).min(count)
                ),
            ));
            if self.offset + PAGE_SIZE < count {
                nav = nav.child(self.control("Next packages", &self.next, cx, |v, w, cx| {
                    v.offset += PAGE_SIZE;
                    v.rows[0].focus(w, cx);
                    cx.notify()
                }))
            }
            body = body.child(nav)
        }
        body
    }

    fn dialog(&self, r: &Review, cx: &Context<Self>) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let p = r.plan();
        let title = r.request().title();
        let count = p.install.len() + p.upgrade.len() + p.remove.len();
        let summary = format!(
            "{} · {} download · {}",
            p.index.label(),
            size_text(p.download_size),
            p.disk_change.map_or_else(
                || "Disk change unknown".into(),
                |d| format!(
                    "{}{} on disk",
                    if d < 0 { "−" } else { "+" },
                    size_text(d.unsigned_abs())
                )
            )
        );
        let mut items = div()
            .id("review-scroll")
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        if !self.review_message.is_empty() {
            items = items.child(label("refusal", self.review_message.clone()))
        }
        if let Some(old) = &p.prepared {
            items = items.child(label(
                "already-prepared",
                format!(
                    "Already prepared for shutdown · {} ({} install or upgrade, {} remove)",
                    old.summary, old.install, old.remove
                ),
            ))
        }
        if count == 0 {
            items = items.child(label("empty-plan", "Nothing to change."))
        }
        let changes = [
            ("Install", &p.install),
            ("Upgrade", &p.upgrade),
            ("Remove", &p.remove),
        ]
        .into_iter()
        .flat_map(|(op, list)| list.iter().map(move |c| (op, c)))
        .skip(self.review_offset)
        .take(REVIEW_PAGE);
        for (i, (op, c)) in changes.enumerate() {
            let version = c
                .from
                .as_ref()
                .map_or_else(|| c.version.clone(), |old| format!("{old} → {}", c.version));
            let kind = if c.named {
                "Selected"
            } else if c.requested {
                "Previously requested"
            } else {
                "Dependency"
            };
            let text = format!("{op} {} · {version} · {kind}", c.name);
            items = items.child(
                mundane_ui::text_label(("change", i), text)
                    .flex()
                    .gap_2()
                    .items_center()
                    .px_2()
                    .py_1()
                    .bg(rgb(theme.surface))
                    .child(
                        div()
                            .w(rems(5.5))
                            .text_color(rgb(theme.secondary))
                            .child(op),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(c.name.clone()),
                    )
                    .child(version)
                    .child(
                        div()
                            .w(rems(7.5))
                            .text_right()
                            .text_sm()
                            .text_color(rgb(theme.secondary))
                            .child(kind),
                    ),
            )
        }
        let mut controls = div().flex().gap_2().justify_end();
        if self.review_offset > 0 {
            controls = controls.child(self.control(
                "Previous changes",
                &self.dialog_prev,
                cx,
                |v, w, cx| {
                    v.review_offset -= REVIEW_PAGE;
                    v.cancel.focus(w, cx);
                    cx.notify()
                },
            ))
        }
        if self.review_offset + REVIEW_PAGE < count {
            controls =
                controls.child(
                    self.control("More changes", &self.dialog_next, cx, |v, w, cx| {
                        v.review_offset += REVIEW_PAGE;
                        v.cancel.focus(w, cx);
                        cx.notify()
                    }),
                )
        }
        controls = controls.child(self.control("Cancel", &self.cancel, cx, |v, w, cx| {
            v.finish(false, w, cx)
        }));
        if p.digest.is_some() {
            controls = controls.child(self.control(
                "Apply changes",
                &self.confirm,
                cx,
                |v, w, cx| v.finish(true, w, cx),
            ))
        }
        Role::Dialog
            .control("plan-dialog", "Review package changes")
            .aria_description(format!("{title}. {summary}"))
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000090))
            .flex()
            .items_center()
            .justify_center()
            .p_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .w(rems(37.))
                    .max_w_full()
                    .max_h_full()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(theme.border))
                    .bg(rgb(theme.panel))
                    .child(
                        label("review-title", title)
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(
                        label("summary", summary)
                            .text_sm()
                            .text_color(rgb(theme.secondary)),
                    )
                    .child(items)
                    .child(controls),
            )
    }
}
fn names(list: &[String]) -> String {
    if list.is_empty() {
        "None".into()
    } else {
        list.join(", ")
    }
}
fn deployment(id: Option<u32>) -> String {
    id.map_or_else(|| "Unknown".into(), |n| n.to_string())
}
impl Render for Packages {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let field = self.field.focus_handle(cx);
        let nav = div()
            .flex()
            .gap_2()
            .child(self.control("Installed", &self.installed, cx, |v, w, cx| {
                v.start(Job::Installed, w, cx)
            }))
            .child(self.control("Discover", &self.discover, cx, Self::explore))
            .child(self.control("Updates", &self.updates, cx, |v, w, cx| {
                v.start(Job::Status, w, cx)
            }));
        let body = self
            .content
            .render(self.review.is_some())
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme.surface))
            .child(self.header.render("Packages", nav, &field, theme, w))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .flex()
                    .items_center()
                    .child(div().flex_1().child(self.field.clone()))
                    .child(self.control("Search", &self.submit, cx, Self::search)),
            )
            .child(
                div()
                    .id("package-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(self.page(cx)),
            )
            .when(!self.notice.is_empty(), |el| {
                el.child(
                    Role::Status
                        .control("status", self.notice.clone())
                        .px_3()
                        .py_2()
                        .bg(rgb(theme.panel))
                        .child(self.notice.clone()),
                )
            });
        div()
            .relative()
            .size_full()
            .font_family("DejaVu Sans")
            .text_color(rgb(theme.text))
            .on_key_down(cx.listener(Self::keys))
            .child(body)
            .children(self.review.as_ref().map(|r| self.dialog(r, cx)))
    }
}
