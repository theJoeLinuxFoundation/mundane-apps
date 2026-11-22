use super::*;
use mundane_ui::{ConfirmDialog, Confirmation};
impl Manager {
    pub(super) fn pick_inputs(&mut self, create: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.confirm.is_some() || (!create && self.archive.is_none()) {
            return;
        }
        let request = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add files".into()),
        });
        self.busy = true;
        cx.notify();
        cx.spawn_in(w, async move |v, cx| {
            let chosen = request.await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.busy = false;
                match chosen {
                    Ok(Ok(Some(files))) if !files.is_empty() => {
                        if create {
                            v.destination(files, w, cx)
                        } else if let Some(path) = v.archive.clone() {
                            v.write(path, v.stamp, files, BTreeSet::new(), w, cx);
                        }
                    }
                    Ok(Ok(_)) => {}
                    _ => v.status = "Could not open the file chooser.".into(),
                }
                cx.notify()
            });
        })
        .detach();
    }
    fn destination(&mut self, files: Vec<PathBuf>, w: &mut Window, cx: &mut Context<Self>) {
        let dir = files[0]
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let request = cx.prompt_for_new_path(dir, Some("Archive.zip"));
        self.busy = true;
        cx.spawn_in(w, async move |v, cx| {
            let chosen = request.await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.busy = false;
                match chosen {
                    Ok(Ok(Some(path))) => v.write(path, None, files, BTreeSet::new(), w, cx),
                    Ok(Ok(None)) => {}
                    _ => v.status = "Could not choose an archive destination.".into(),
                }
                cx.notify()
            });
        })
        .detach()
    }
    pub(super) fn remove(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.confirm.is_some() {
            return;
        }
        let Some(path) = self.archive.clone() else {
            return;
        };
        let mut remove = self.marked.clone();
        if remove.is_empty()
            && let Some(i) = self.list.selected
        {
            remove.insert(self.entries[i].path.clone());
        }
        if remove.is_empty() {
            return;
        }
        let Some(stamp) = self.stamp else { return };
        let description = if remove.len() == 1 {
            remove.first().unwrap().display().to_string()
        } else {
            format!("{} marked entries", remove.len())
        };
        let dialog=cx.new(|cx|ConfirmDialog::new("Remove archive entries",format!("Remove {description} from this archive? Selected folders include their contents."),"Remove",cx));
        cx.subscribe_in(&dialog, w, move |v, _, answer: &Confirmation, w, cx| {
            v.confirm = None;
            if *answer == Confirmation::Confirmed {
                v.write(path.clone(), Some(stamp), vec![], remove.clone(), w, cx);
            }
            cx.notify()
        })
        .detach();
        dialog.update(cx, |d, cx| d.show(&self.list.focus, w, cx));
        self.confirm = Some(dialog);
        cx.notify()
    }
    fn write(
        &mut self,
        path: PathBuf,
        stamp: Option<archive::Stamp>,
        files: Vec<PathBuf>,
        remove: BTreeSet<PathBuf>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.status = "Writing archive…".into();
        cx.notify();
        let target = path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { archive::edit(&target, stamp, &files, &remove) });
        cx.spawn_in(w, async move |v, cx| {
            let result = task.await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.busy = false;
                match result {
                    Ok(()) => v.open(path, w, cx),
                    Err(e) => v.status = format!("Could not write archive. {e:#}"),
                }
                cx.notify()
            });
        })
        .detach();
    }
}
