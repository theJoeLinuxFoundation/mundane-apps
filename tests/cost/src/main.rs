//! Measure synchronous GPUI frame construction on an isolated Wayland window.
use gpui::{prelude::*, *};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    io::Write,
    time::Duration,
};

thread_local! { static ALLOCS: Cell<u64> = const { Cell::new(0) }; }
struct Count;
#[global_allocator]
static COUNT: Count = Count;
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.with(|n| n.set(n.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
fn cpu(clock: libc::clockid_t) -> u64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(unsafe { libc::clock_gettime(clock, &mut t) }, 0);
    (t.tv_sec * 1_000_000_000 + t.tv_nsec) as u64
}
const VISIBLE: usize = 20;
struct Rows {
    model: Vec<SharedString>,
    top: usize,
}
impl Render for Rows {
    fn render(&mut self, _window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let body = div().id("rows").flex().flex_col();
        #[cfg(feature = "bridge")]
        let body = body
            .role(Role::ListBox)
            .aria_label("Rows")
            .aria_size_of_set(self.model.len());
        let mut body = body;
        for i in self.top..self.top + VISIBLE {
            let row = div().id(i).h(px(20.)).child(self.model[i].clone());
            #[cfg(feature = "trace")]
            let before = ALLOCS.with(Cell::get);
            #[cfg(feature = "bridge")]
            let row = row
                .role(Role::ListBoxOption)
                .aria_label(self.model[i].clone())
                .aria_selected(i == self.top)
                .aria_position_in_set(i);
            #[cfg(feature = "trace")]
            SEMANTIC_ALLOCS.fetch_add(ALLOCS.with(Cell::get) - before, Relaxed);
            body = body.child(row);
        }
        body
    }
}
struct Fixture {
    rows: Entity<Rows>,
    caret: bool,
}
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(
                AnyView::from(self.rows.clone())
                    .cached(StyleRefinement::default().w_full().h(px(400.))),
            )
            .child(
                div()
                    .size(px(1.))
                    .bg(rgb(if self.caret { 0xffffff } else { 0 })),
            )
    }
}

// Called only by the separately instrumented diagnostic build.
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
static ADAPTER_CLOCK: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
pub extern "C" fn mundane_cost_adapter_thread() {
    let mut clock = 0;
    assert_eq!(
        unsafe { libc::pthread_getcpuclockid(libc::pthread_self(), &mut clock) },
        0
    );
    ADAPTER_CLOCK.store(clock as u64, Relaxed);
}
fn adapter_cpu() -> u64 {
    let clock = ADAPTER_CLOCK.load(Relaxed) as libc::clockid_t;
    if clock == 0 { 0 } else { cpu(clock) }
}
static SEMANTIC_ALLOCS: AtomicU64 = AtomicU64::new(0);
static ACTIVATION: AtomicU64 = AtomicU64::new(0);
static FIRST_NS: AtomicU64 = AtomicU64::new(0);
static NODES: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static UPDATES: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
pub extern "C" fn mundane_cost_activation() {
    ACTIVATION.store(cpu(libc::CLOCK_MONOTONIC), Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn mundane_cost_update(nodes: usize, json_bytes: usize) {
    if UPDATES.fetch_add(1, Relaxed) == 0 {
        FIRST_NS.store(
            cpu(libc::CLOCK_MONOTONIC) - ACTIVATION.load(Relaxed),
            Relaxed,
        );
    }
    NODES.fetch_add(nodes as u64, Relaxed);
    BYTES.fetch_add(json_bytes as u64, Relaxed);
}
fn main() {
    Application::with_platform(gpui_linux::current_platform(false)).run(|cx| {
        let window = cx
            .open_window(
                WindowOptions {
                    app_id: Some("mundane.cost".into()),
                    ..Default::default()
                },
                |_, cx| {
                    let rows = cx.new(|_| Rows {
                        model: (0..100000).map(|i| format!("row {i}").into()).collect(),
                        top: 0,
                    });
                    cx.new(|_| Fixture { rows, caret: false })
                },
            )
            .unwrap();
        cx.spawn(async move |cx| {
            // Let Wayland configure the surface and the private registry activate the adapter.
            cx.background_executor()
                .timer(Duration::from_millis(800))
                .await;
            let mut out = std::fs::File::create(std::env::var_os("COST_OUTPUT").unwrap()).unwrap();
            if std::env::var_os("COST_ACTIVATION_ONLY").is_some() {
                let first = FIRST_NS.load(Relaxed);
                assert!(first > 0);
                writeln!(out, "{first}").unwrap();
                cx.update(|cx| cx.quit());
                return;
            }
            #[cfg(feature = "bridge")]
            let active = std::env::var_os("EXPECT_ACTIVE").is_some();
            let mut trace =
                std::env::var_os("COST_TRACE").map(|p| std::fs::File::create(p).unwrap());
            for op in ["cached", "blink", "edit", "scroll"] {
                let semantic_start = SEMANTIC_ALLOCS.load(Relaxed);
                let phase_start = cpu(libc::CLOCK_REALTIME);
                let adapter_start = adapter_cpu();
                let before = (
                    UPDATES.load(Relaxed),
                    NODES.load(Relaxed),
                    BYTES.load(Relaxed),
                );
                for frame in 0..1050 {
                    let start = cpu(libc::CLOCK_THREAD_CPUTIME_ID);
                    let process = cpu(libc::CLOCK_PROCESS_CPUTIME_ID);
                    let alloc = ALLOCS.with(Cell::get);
                    window
                        .update(cx, |model, w, cx| {
                            match op {
                                "blink" => {
                                    model.caret = !model.caret;
                                    cx.notify();
                                }
                                "edit" | "scroll" => model.rows.update(cx, |r, cx| {
                                    if op == "scroll" {
                                        r.top += 1;
                                    } else {
                                        r.model[r.top] = if frame % 2 == 0 {
                                            "mutation A".into()
                                        } else {
                                            "mutation B".into()
                                        };
                                    }
                                    cx.notify();
                                }),
                                _ => {}
                            }
                            #[cfg(feature = "bridge")]
                            assert_eq!(w.is_a11y_active(), active);
                        })
                        .unwrap();
                    cx.update(|cx| {
                        cx.update_window(window.into(), |_, w, cx| {
                            let guard = w.draw(cx);
                            #[cfg(feature = "bridge")]
                            guard.clear(cx);
                            #[cfg(not(feature = "bridge"))]
                            guard.clear();
                        })
                        .unwrap()
                    });
                    let elapsed = cpu(libc::CLOCK_THREAD_CPUTIME_ID) - start;
                    let total = cpu(libc::CLOCK_PROCESS_CPUTIME_ID) - process;
                    let allocations = ALLOCS.with(Cell::get) - alloc;
                    if frame >= 50 {
                        writeln!(out, "{op},{elapsed},{total},{allocations}").unwrap();
                    }
                    if frame % 100 == 99 {
                        cx.background_executor()
                            .timer(Duration::from_millis(1))
                            .await;
                    }
                }
                if let Some(trace) = &mut trace {
                    writeln!(
                        trace,
                        "{op},{},{},{},{},{},{},{},{}",
                        UPDATES.load(Relaxed) - before.0,
                        NODES.load(Relaxed) - before.1,
                        BYTES.load(Relaxed) - before.2,
                        FIRST_NS.load(Relaxed),
                        phase_start,
                        cpu(libc::CLOCK_REALTIME),
                        adapter_cpu() - adapter_start,
                        SEMANTIC_ALLOCS.load(Relaxed) - semantic_start
                    )
                    .unwrap();
                }
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
