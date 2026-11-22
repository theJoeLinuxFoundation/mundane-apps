mod arithmetic;
#[cfg(test)]
mod tests;
mod view;

use gpui::{prelude::*, *};
use mundane_ui::{TextField, Theme, WindowHeader};

const APP_ID: &str = "org.joelinux.mundane.Calculator";
const HISTORY_LIMIT: usize = 50;

struct Entry {
    expression: String,
    answer: String,
    focus: FocusHandle,
}

struct Calculator {
    field: Entity<TextField>,
    evaluated: Option<(String, Result<String, &'static str>)>,
    history: Vec<Entry>, // Newest first.
    keys: [FocusHandle; 20],
    reuse: FocusHandle,
    clear: FocusHandle,
    header: WindowHeader,
    scroll: ScrollHandle,
    last_focus: Option<FocusHandle>,
}

impl Calculator {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let theme = Theme::current(cx);
        window.set_window_title("Calculator");
        let field = cx.new(|cx| TextField::new("Expression", theme, cx));
        field.read(cx).focus_handle(cx).focus(window, cx);
        cx.observe(&field, |_, _, cx| cx.notify()).detach();
        Self {
            field,
            evaluated: None,
            history: Vec::new(),
            keys: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            reuse: cx.focus_handle().tab_stop(true),
            clear: cx.focus_handle().tab_stop(true),
            header: WindowHeader::new(cx),
            scroll: ScrollHandle::new(),
            last_focus: None,
        }
    }

    fn result(&self, cx: &App) -> Option<Result<&str, &str>> {
        let (source, answer) = self.evaluated.as_ref()?;
        (self.field.read(cx).text() == source).then(|| answer.as_deref().map_err(|e| *e))
    }

    fn evaluate(&mut self, cx: &mut Context<Self>) {
        let expression = self.field.read(cx).text().to_owned();
        let answer = arithmetic::evaluate(&expression);
        if let Ok(answer) = &answer {
            self.history.insert(
                0,
                Entry {
                    expression: expression.clone(),
                    answer: answer.clone(),
                    focus: cx.focus_handle().tab_stop(true),
                },
            );
            self.history.truncate(HISTORY_LIMIT);
        }
        self.evaluated = Some((expression, answer));
        cx.notify();
    }

    fn edit(
        &mut self,
        input: &str,
        replace_all: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.field.update(cx, |field, cx| {
            if replace_all {
                field.select_match(0..field.text().len(), cx);
            }
            field.replace_text_in_range(None, input, window, cx);
            field.focus_handle(cx).focus(window, cx);
        });
        self.evaluated = None;
        cx.notify();
    }

    fn use_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Ok(value)) = self.result(cx) {
            self.edit(&arithmetic::operand(value), true, window, cx);
        }
    }

    fn clear_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.history.clear();
        self.field.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn activate_key(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        match KEYS[i].0 {
            "=" => self.evaluate(cx),
            "Clear" => self.edit("", true, window, cx),
            "⌫" => self
                .field
                .update(cx, |field, cx| field.delete_backward(window, cx)),
            text => self.edit(text, false, window, cx),
        }
        self.field.read(cx).focus_handle(cx).focus(window, cx);
    }

    fn keyboard(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let field = self.field.read(cx).focus_handle(cx);
        match key.key.as_str() {
            "enter" if field.is_focused(window) => self.evaluate(cx),
            "r" if key.modifiers.control => self.use_result(window, cx),
            "l" if key.modifiers.control => self.edit("", true, window, cx),
            "w" if key.modifiers.control => window.remove_window(),
            "h" if key.modifiers.alt => {
                if let Some(entry) = self.history.first() {
                    entry.focus.focus(window, cx);
                }
            }
            "f6" => self.reuse.focus(window, cx),
            "escape" => field.focus(window, cx),
            "tab" if key.modifiers.shift => window.focus_prev(cx),
            "tab" => window.focus_next(cx),
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
    }
}

const KEYS: [(&str, &str); 20] = [
    ("(", "Open parenthesis"),
    (")", "Close parenthesis"),
    ("⌫", "Backspace"),
    ("÷", "Divide"),
    ("7", "7"),
    ("8", "8"),
    ("9", "9"),
    ("×", "Multiply"),
    ("4", "4"),
    ("5", "5"),
    ("6", "6"),
    ("−", "Subtract"),
    ("1", "1"),
    ("2", "2"),
    ("3", "3"),
    ("+", "Add"),
    ("Clear", "Clear expression"),
    ("0", "0"),
    (".", "Decimal point"),
    ("=", "Equals"),
];

fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("Calculator requires a Wayland session.");
        std::process::exit(1);
    }
    mundane_ui::run(|cx| {
        mundane_ui::bind_text_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(424.), px(594.)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                app_id: Some(APP_ID.into()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Calculator::new(window, cx)),
        ) {
            eprintln!("Could not open Calculator. {error}");
            std::process::exit(1);
        }
    });
}
