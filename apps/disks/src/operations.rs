use super::*;
use mundane_services::disks::{DeviceAction, Operation};
use mundane_ui::{ConfirmDialog, Confirmation, TextField};

pub(super) enum Prompt {
    Drive(Entity<ConfirmDialog>),
    Unlock(DeviceAction, Entity<TextField>),
}
impl Disks {
    pub(super) fn ask(&mut self, target: DeviceAction, w: &mut Window, cx: &mut Context<Self>) {
        if self.changing || self.prompt.is_some() {
            return;
        }
        let Some(item) = self
            .list
            .selected
            .and_then(|i| self.items.get(i))
            .filter(|n| n.actions.contains(&target))
        else {
            return;
        };
        let label = item.label.clone();
        if target.operation == Operation::Unlock {
            let theme = Theme::current(cx);
            let field = cx.new(|cx| TextField::password("Disk passphrase", theme, cx));
            field.focus_handle(cx).focus(w, cx);
            self.prompt = Some(Prompt::Unlock(target, field));
        } else {
            let action = target.operation.label();
            let (subject, message) = match target.operation {
                Operation::Lock => (
                    "volume",
                    format!("Lock {label}? Its decrypted contents will no longer be available."),
                ),
                Operation::PowerOff => (
                    "drive",
                    format!(
                        "Safely power off {label}? Other drives in the same enclosure may also disconnect."
                    ),
                ),
                Operation::Eject => ("drive", format!("Eject the media from {label}?")),
                Operation::Unlock => unreachable!(),
            };
            let dialog =
                cx.new(|cx| ConfirmDialog::new(format!("{action} {subject}"), message, action, cx));
            cx.subscribe_in(&dialog, w, move |this, _, answer: &Confirmation, w, cx| {
                this.prompt = None;
                if *answer == Confirmation::Confirmed {
                    this.perform(target.clone(), String::new(), w, cx);
                }
                cx.notify()
            })
            .detach();
            dialog.update(cx, |d, cx| d.show(&self.list.focus, w, cx));
            self.prompt = Some(Prompt::Drive(dialog));
        }
        cx.notify()
    }
    fn perform(
        &mut self,
        target: DeviceAction,
        secret: String,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_change(move |service| service.perform(&target, &secret), w, cx);
    }
    pub(super) fn unlock(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(Prompt::Unlock(target, field)) = &self.prompt {
            let secret = field.read(cx).text().to_owned();
            if secret.is_empty() {
                return;
            }
            let target = target.clone();
            self.prompt = None;
            self.list.focus.focus(w, cx);
            self.perform(target, secret, w, cx);
        }
    }
    pub(super) fn overlay(&self, cx: &Context<Self>) -> AnyElement {
        match self.prompt.as_ref() {
            Some(Prompt::Drive(d)) => d.clone().into_any_element(),
            Some(Prompt::Unlock(_, field)) => {
                let t = Theme::current(cx);
                let v = cx.weak_entity();
                let accept = cx.weak_entity();
                Role::Dialog
                    .control("unlock-volume", "Unlock encrypted volume")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgba(0x00000070))
                    .child(
                        div()
                            .w(rems(26.))
                            .p_4()
                            .rounded_md()
                            .bg(rgb(t.panel))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(mundane_ui::heading(
                                "unlock-heading",
                                "Unlock encrypted volume",
                            ))
                            .child(field.clone())
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .justify_end()
                                    .child(button("Cancel", &self.cancel, t, move |w, cx| {
                                        let _ = v.update(cx, |s, cx| {
                                            s.prompt = None;
                                            s.list.focus.focus(w, cx);
                                            cx.notify()
                                        });
                                    }))
                                    .child(button(
                                        "Unlock",
                                        &self.unlock_button,
                                        t,
                                        move |w, cx| {
                                            let _ = accept.update(cx, |s, cx| s.unlock(w, cx));
                                        },
                                    )),
                            ),
                    )
                    .into_any_element()
            }
            None => div().into_any_element(),
        }
    }
}
