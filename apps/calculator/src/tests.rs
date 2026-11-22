use super::Calculator;
use gpui::{Focusable, TestAppContext};

#[gpui::test]
fn evaluate_reuse_and_clear_by_keyboard(cx: &mut TestAppContext) {
    cx.update(mundane_ui::bind_text_keys);
    let (view, cx) = cx.add_window_view(Calculator::new);
    cx.simulate_keystrokes("0 . 1 + 0 . 2 enter");
    cx.update(|_, cx| assert_eq!(view.read(cx).result(cx), Some(Ok("0.3"))));
    cx.simulate_keystrokes("ctrl-r * 2 enter");
    cx.update(|_, cx| {
        let calc = view.read(cx);
        assert_eq!(calc.result(cx), Some(Ok("0.6")));
        assert_eq!(calc.history.len(), 2);
    });
    cx.simulate_keystrokes("ctrl-l 1 / 0 enter");
    cx.update(|_, cx| {
        assert_eq!(
            view.read(cx).result(cx),
            Some(Err("Cannot divide by zero."))
        )
    });
    cx.simulate_keystrokes("alt-h tab enter");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.update(|w, cx| {
        let calc = view.read(cx);
        assert_eq!(calc.field.read(cx).text(), "0.3");
        assert!(calc.field.read(cx).focus_handle(cx).is_focused(w));
    });
    cx.simulate_keystrokes("alt-h shift-tab enter");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.update(|w, cx| {
        let calc = view.read(cx);
        assert!(calc.history.is_empty());
        assert!(calc.field.read(cx).focus_handle(cx).is_focused(w));
    });
}
