use super::*;
use mundane_ui::{ControlRole, launcher_row};
use std::collections::BTreeSet;

#[derive(Clone)]
enum Row {
    Category(String),
    Application(usize),
}
impl Packages {
    pub(super) fn explore(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.review.is_some() {
            return;
        }
        self.category = None;
        self.app_index = None;
        self.preview_image = None;
        self.start(Job::Discover, w, cx)
    }
    pub(super) fn catalog_back(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.app_index.take().is_none() {
            self.category = None;
        }
        self.preview_image = None;
        self.notice.clear();
        self.offset = 0;
        self.rows[0].focus(w, cx);
        cx.notify();
    }
    pub(super) fn catalog_page(
        &self,
        apps: &[catalog::Application],
        mut body: Div,
        cx: &Context<Self>,
    ) -> Div {
        let t = Theme::current(cx);
        if self.category.is_some() {
            body = body.child(div().flex().child(self.control(
                "Back",
                &self.back,
                cx,
                Self::catalog_back,
            )));
        }
        if let Some(a) = self.app_index.and_then(|i| apps.get(i)) {
            body = body
                .child(mundane_ui::heading("app-name", a.name.clone()).text_xl())
                .child(super::view::label("summary", a.summary.clone()))
                .child(super::view::label("categories", a.categories.join(" · ")));
            if !a.description.is_empty() {
                body = body.child(super::view::label("app-description", a.description.clone()));
            }
            let mut actions = div().flex().flex_wrap().gap_2();
            if a.installed
                && let Some(id) = a.desktop.clone()
            {
                actions =
                    actions.child(self.control("Launch", &self.launch, cx, move |v, w, cx| {
                        v.start(Job::Launch(id.clone()), w, cx)
                    }));
            }
            if let Some(pkg) = a.package.clone() {
                actions = actions.child(self.control(
                    "Package details",
                    &self.action,
                    cx,
                    move |v, w, cx| v.start(Job::Info(pkg.clone()), w, cx),
                ));
            }
            if let Some((url, _)) = a.screenshot.clone() {
                actions = actions.child(self.control(
                    "Show screenshot",
                    &self.picture,
                    cx,
                    move |v, w, cx| v.start(Job::Screenshot(url.clone()), w, cx),
                ));
            }
            body = body.child(actions);
            if let (Some(pic), Some((_, caption))) = (&self.preview_image, &a.screenshot) {
                body = body
                    .child(
                        Role::Image
                            .control("app-screenshot", caption.clone())
                            .child(
                                img(pic.clone())
                                    .w_full()
                                    .h(rems(9.))
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(super::view::label("caption", caption.clone()));
            }
            return body;
        }
        let title = self.category.as_deref().unwrap_or("Discover applications");
        body = body.child(mundane_ui::heading("discover-heading", title.to_owned()).text_xl());
        let rows: Vec<_> = if let Some(category) = &self.category {
            apps.iter()
                .enumerate()
                .filter(|(_, a)| a.categories.contains(category))
                .map(|(i, a)| (Row::Application(i), a.name.clone(), a.summary.clone()))
                .collect()
        } else {
            let groups: BTreeSet<_> = apps.iter().flat_map(|a| a.categories.iter()).collect();
            groups
                .into_iter()
                .map(|g| {
                    let n = apps.iter().filter(|a| a.categories.contains(g)).count();
                    (
                        Row::Category(g.clone()),
                        g.clone(),
                        format!(
                            "{n} {}",
                            if n == 1 {
                                "application"
                            } else {
                                "applications"
                            }
                        ),
                    )
                })
                .collect()
        };
        if rows.is_empty() {
            return body.child(super::view::label(
                "empty-discover",
                "No applications found in the local catalog or desktop entries.",
            ));
        }
        let count = rows.len();
        for (i, (row, name, detail)) in rows
            .into_iter()
            .skip(self.offset)
            .take(PAGE_SIZE)
            .enumerate()
        {
            let view = cx.weak_entity();
            body = body.child(
                launcher_row(
                    name.clone(),
                    &self.rows[i],
                    false,
                    !self.busy,
                    t,
                    move |w, cx| {
                        let _ = view.update(cx, |v, cx| {
                            if v.busy {
                                return;
                            }
                            match &row {
                                Row::Category(c) => {
                                    v.category = Some(c.clone());
                                    v.offset = 0;
                                    v.rows[0].focus(w, cx);
                                }
                                Row::Application(i) => {
                                    v.app_index = Some(*i);
                                    v.back.focus(w, cx);
                                }
                            }
                            v.scroll.set_offset(Default::default());
                            cx.notify();
                        });
                    },
                )
                .aria_description(detail.clone())
                .px_3()
                .py_1()
                .flex()
                .flex_col()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                .child(div().text_sm().child(detail)),
            );
        }
        self.pagination(body, count, cx)
    }
}
