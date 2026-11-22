use super::*;
use mundane_ui::{ControlRole, Role};

impl Monitor {
    pub(super) fn resources(&self, cx: &App) -> Div {
        let t = Theme::current(cx);
        let last = self
            .samples
            .back()
            .map(|(_, usage)| *usage)
            .unwrap_or_default();
        let span = self
            .samples
            .front()
            .zip(self.samples.back())
            .map(|((start, _), (end, _))| end.saturating_duration_since(*start).as_secs_f32())
            .unwrap_or(0.);
        let times: Arc<Vec<_>> = Arc::new(
            self.samples
                .iter()
                .map(|(at, _)| {
                    at.saturating_duration_since(self.samples[0].0)
                        .as_secs_f32()
                        / span.max(0.001)
                })
                .collect(),
        );
        let period = if span < 1. {
            "Collecting samples · Updated every 2 seconds".into()
        } else {
            format!("Last {span:.0} seconds · Updated every 2 seconds")
        };
        let cpu = last.cpu.map(|p| format!("{p:.1}%"));
        let memory = last.memory.map(|(used, total)| {
            format!(
                "{} of {}",
                super::view::memory(used),
                super::view::memory(total)
            )
        });
        let mut body = div()
            .id("history-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .child(mundane_ui::heading("history", "Resource history").text_lg())
            .child(mundane_ui::text_label("interval", period.clone()).child(period));
        for (label, value, points, percent) in [
            (
                "CPU",
                cpu,
                self.samples.iter().map(|(_, s)| s.cpu).collect::<Vec<_>>(),
                true,
            ),
            (
                "Memory",
                memory,
                self.samples
                    .iter()
                    .map(|(_, s)| s.memory.map(|(u, t)| 100. * u as f64 / t.max(1) as f64))
                    .collect(),
                true,
            ),
            (
                "Network received",
                last.network.map(|(r, _)| rate(r)),
                self.samples
                    .iter()
                    .map(|(_, s)| s.network.map(|n| n.0))
                    .collect(),
                false,
            ),
            (
                "Network sent",
                last.network.map(|(_, t)| rate(t)),
                self.samples
                    .iter()
                    .map(|(_, s)| s.network.map(|n| n.1))
                    .collect(),
                false,
            ),
        ] {
            let maximum = if percent {
                100.
            } else {
                points.iter().flatten().copied().fold(1024., f64::max)
            };
            let status = value.unwrap_or_else(|| "Awaiting sample".into());
            let scale = if percent {
                "0–100%".into()
            } else {
                format!("0–{}", rate(maximum))
            };
            let times = times.clone();
            let description = format!("{label}: {status}. Scale {scale}");
            body = body.child(
                Role::Figure
                    .control(label, description)
                    .flex_none()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(t.border))
                    .p_2()
                    .bg(rgb(t.panel))
                    .child(div().flex().justify_between().child(label).child(status))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |b, (), w, _| {
                                let mut line = PathBuilder::stroke(px(2.));
                                let mut connected = false;
                                for (i, v) in points.iter().enumerate() {
                                    if let Some(v) = v {
                                        let x = b.left() + b.size.width * times[i];
                                        let y = b.bottom()
                                            - b.size.height * (v / maximum).clamp(0., 1.) as f32;
                                        if connected {
                                            line.line_to(point(x, y))
                                        } else {
                                            line.move_to(point(x, y));
                                        }
                                        w.paint_quad(fill(
                                            Bounds::new(
                                                point(x - px(1.), y - px(1.)),
                                                size(px(2.), px(2.)),
                                            ),
                                            rgb(t.focus),
                                        ));
                                        connected = true;
                                    } else {
                                        connected = false;
                                    }
                                }
                                if let Ok(path) = line.build() {
                                    w.paint_path(path, rgb(t.focus));
                                }
                            },
                        )
                        .h(px(24.))
                        .w_full(),
                    )
                    .child(div().text_xs().text_color(rgb(t.secondary)).child(scale)),
            );
        }
        div().flex().flex_col().flex_1().min_h_0().child(body)
    }
}
fn rate(v: f64) -> String {
    format!(
        "{}/s",
        if v < 1024. {
            format!("{:.0} B", v)
        } else {
            super::view::memory(v as u64)
        }
    )
}
