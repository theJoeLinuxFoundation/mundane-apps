//! Real shared controls in two windows, including a password field absent from the apps.
use gpui::{prelude::*, *};
use mundane_ui::{TextField, Theme, button, unavailable};
use std::io::Write;

struct Fields {
    field: Entity<TextField>,
    button: FocusHandle,
    password: bool,
}
impl Render for Fields {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let password = self.password;
        let command = button("Close fixture", &self.button, Theme::DARK, move |w, _| {
            if password {
                return;
            }
            let path = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
                .join("accepted");
            writeln!(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .unwrap(),
                "close"
            )
            .unwrap();
            w.remove_window();
        });
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.field.clone())
            .child(unavailable(command, password))
    }
}
fn main() {
    Application::with_platform(gpui_linux::current_platform(false)).run(|cx| {
        mundane_ui::bind_text_keys(cx);
        for (title, password) in [("Document fixture", false), ("Password fixture", true)] {
            cx.open_window(
                WindowOptions {
                    app_id: Some("mundane.fields".into()),
                    ..Default::default()
                },
                move |w, cx| {
                    w.set_window_title(title);
                    let field = cx.new(|cx| {
                        if password {
                            TextField::password("Passphrase", Theme::DARK, cx)
                        } else {
                            let mut field = TextField::multiline("Notes", Theme::DARK, cx);
                            field.set_document("e\u{301}😀\r\nline two".into(), cx);
                            field
                        }
                    });
                    field.read(cx).focus_handle(cx).focus(w, cx);
                    cx.new(|cx| Fields {
                        field,
                        password,
                        button: cx.focus_handle().tab_stop(true),
                    })
                },
            )
            .unwrap();
        }
    });
}
