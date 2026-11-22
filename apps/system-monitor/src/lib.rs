#[cfg(test)]
mod tests;
mod view;

use gpui::{prelude::*, *};
use mundane_services::processes::{History, Process, ProcessId, ProcessService, Sample};
use mundane_ui::{Theme, WindowHeader};
use std::{cmp::Ordering, io, sync::Arc, time::Duration};

const INTERVAL: Duration = Duration::from_secs(2);
const APP_ID: &str = "org.joelinux.mundane.SystemMonitor";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Name,
    Pid,
    Cpu,
    Memory,
}

impl Column {
    const ALL: [Column; 4] = [Self::Name, Self::Pid, Self::Cpu, Self::Memory];
    fn name(self) -> &'static str {
        match self {
            Self::Name => "Process",
            Self::Pid => "PID",
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
        }
    }
}

struct Confirmation {
    target: ProcessId,
    name: String,
    cancel: FocusHandle,
    confirm: FocusHandle,
    return_to: FocusHandle,
}

struct Monitor {
    service: Arc<dyn ProcessService>,
    _sampling: Task<()>,
    history: History,
    rows: Vec<Process>,
    selected: Option<ProcessId>,
    column: Column,
    descending: bool,
    confirm: Option<Confirmation>,
    sending: bool,
    sample_error: Option<String>,
    status: Option<String>,
    header: WindowHeader,
    content: mundane_ui::ApplicationContent,
    heading_focus: [FocusHandle; 4],
    end_focus: FocusHandle,
    list_focus: FocusHandle,
    scroll: UniformListScrollHandle,
    text_size: f32,
    theme: Theme,
}

impl Monitor {
    fn new(service: Arc<dyn ProcessService>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list_focus = cx.focus_handle().tab_stop(true);
        list_focus.focus(window, cx);
        let sampler = service.clone();
        let executor = cx.background_executor().clone();
        let sampling = cx.spawn_in(window, async move |view, cx| {
            loop {
                let source = sampler.clone();
                let sample = executor.spawn(async move { source.sample() }).await;
                if view
                    .update_in(cx, |this: &mut Self, window, cx| {
                        this.refresh(sample, window, cx)
                    })
                    .is_err()
                {
                    break;
                }
                executor.timer(INTERVAL).await;
            }
        });
        Self {
            service,
            _sampling: sampling,
            history: History::default(),
            rows: Vec::new(),
            selected: None,
            column: Column::Name,
            descending: false,
            confirm: None,
            sending: false,
            sample_error: None,
            status: None,
            content: Default::default(),
            header: WindowHeader::new(cx),
            heading_focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            end_focus: cx.focus_handle().tab_stop(true),
            list_focus,
            scroll: UniformListScrollHandle::new(),
            text_size: 16.,
            theme: Theme::current(),
        }
    }

    fn refresh(&mut self, result: io::Result<Sample>, window: &mut Window, cx: &mut Context<Self>) {
        match result {
            Ok(sample) => {
                self.sample_error = None;
                self.rows = self.history.update(sample);
                self.sort_rows();
                if self.selected.is_some() && self.position().is_none() {
                    self.selected = None;
                    if self.status.is_none() {
                        self.status = Some("The selected process is no longer listed.".into());
                    }
                }
            }
            Err(err) => {
                self.sample_error = Some(format!("Could not read processes. {err}"));
                self.rows.clear();
                self.selected = None;
            }
        }
        if let Some(prompt) = &self.confirm
            && self.selected != Some(prompt.target)
            && prompt.confirm.is_focused(window)
        {
            prompt.cancel.focus(window, cx);
        }
        cx.notify();
    }

    fn position(&self) -> Option<usize> {
        let id = self.selected?;
        self.rows.iter().position(|p| p.id == id)
    }

    fn sort_rows(&mut self) {
        self.rows.sort_unstable_by(|a, b| {
            let order = match self.column {
                Column::Name => a.name.cmp(&b.name),
                Column::Pid => a.id.pid.cmp(&b.id.pid),
                Column::Memory => a.resident_bytes.cmp(&b.resident_bytes),
                Column::Cpu => match (a.cpu_percent, b.cpu_percent) {
                    (Some(a), Some(b)) => a.total_cmp(&b),
                    (None, None) => Ordering::Equal,
                    (None, Some(_)) => return Ordering::Greater,
                    (Some(_), None) => return Ordering::Less,
                },
            };
            (if self.descending {
                order.reverse()
            } else {
                order
            })
            .then_with(|| a.id.pid.cmp(&b.id.pid))
        });
    }

    fn sort(&mut self, column: Column, cx: &mut Context<Self>) {
        if self.confirm.is_some() {
            return;
        }
        self.descending = if self.column == column {
            !self.descending
        } else {
            matches!(column, Column::Cpu | Column::Memory)
        };
        self.column = column;
        self.sort_rows();
        if let Some(index) = self.position() {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirm.is_some() {
            return;
        }
        self.selected = Some(self.rows[index].id);
        self.status = None;
        self.list_focus.focus(window, cx);
        self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn ask(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirm.is_some() || self.sending {
            return;
        }
        if let Some(index) = self.position() {
            let p = &self.rows[index];
            let return_to = window
                .focused(cx)
                .unwrap_or_else(|| self.list_focus.clone());
            let cancel = cx.focus_handle();
            cancel.focus(window, cx);
            self.confirm = Some(Confirmation {
                target: p.id,
                name: p.name.clone(),
                cancel,
                confirm: cx.focus_handle(),
                return_to,
            });
        } else {
            self.status = Some("Select a process first.".into());
        }
        cx.notify();
    }

    fn finish(&mut self, yes: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.confirm.take() else {
            return;
        };
        prompt.return_to.focus(window, cx);
        cx.notify();
        if !yes {
            return;
        }
        if self.selected != Some(prompt.target) {
            self.status = Some("The selected process is no longer available.".into());
            return;
        }
        self.sending = true;
        self.status = Some("Sending SIGTERM…".into());
        let source = self.service.clone();
        let id = prompt.target;
        let job = cx
            .background_executor()
            .spawn(async move { source.terminate(id) });
        cx.spawn(async move |view, cx| {
            let result = job.await;
            let _ = view.update(cx, |this, cx| {
                this.sending = false;
                this.status = Some(match result {
                    Ok(()) => format!("Sent SIGTERM to {} (PID {}).", prompt.name, id.pid),
                    Err(e) => format!("Could not end {} (PID {}). {e}", prompt.name, id.pid),
                });
                cx.notify();
            });
        })
        .detach();
    }
}

pub fn run(service: Arc<dyn ProcessService>) {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("System Monitor requires a Wayland session.");
        std::process::exit(1);
    }
    Application::with_platform(gpui_linux::current_platform(false)).run(move |cx| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(760.), px(500.)), cx);
        if let Err(err) = cx.open_window(
            WindowOptions {
                app_id: Some(APP_ID.into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("System Monitor");
                cx.new(|cx| Monitor::new(service, window, cx))
            },
        ) {
            eprintln!("Could not open System Monitor. {err}");
            std::process::exit(1);
        }
    });
}
