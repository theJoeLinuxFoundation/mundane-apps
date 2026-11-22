use super::*;
use anyhow::{Context as _, Result, ensure};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::Arc,
};
use tempfile::NamedTempFile;

pub(super) struct Download {
    file: NamedTempFile,
    pub name: std::ffi::OsString,
}
impl Download {
    pub fn open(path: &Path) -> Result<(Self, Vec<Family>)> {
        let name = path
            .file_name()
            .context("Missing font filename")?
            .to_owned();
        let source = File::options()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open(path)?;
        const LIMIT: u64 = 128 * 1024 * 1024;
        let meta = source.metadata()?;
        ensure!(
            meta.is_file() && meta.len() <= LIMIT,
            "Choose a regular font file under 128 MiB."
        );
        let mut file = NamedTempFile::new()?;
        ensure!(
            std::io::copy(&mut source.take(LIMIT + 1), &mut file)? <= LIMIT,
            "Font is too large."
        );
        file.flush()?;
        let families = catalog::installed(Some(file.path())).map_err(anyhow::Error::msg)?;
        ensure!(!families.is_empty(), "No usable faces in this font file.");
        Ok((Self { file, name }, families))
    }
    pub fn install(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let dest = dir.join(&self.name);
        let mut tmp = NamedTempFile::new_in(dir)?;
        std::io::copy(&mut self.file.reopen()?, &mut tmp)?;
        tmp.flush()?;
        match tmp.persist_noclobber(&dest) {
            Ok(_) => Ok(()),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                let mut existing = File::options()
                    .read(true)
                    .custom_flags(
                        (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
                    )
                    .open(&dest)?;
                let mut source = self.file.reopen()?;
                ensure!(
                    existing.metadata()?.is_file()
                        && existing.metadata()?.len() == source.metadata()?.len(),
                    "A different file already uses this name."
                );
                let (mut a, mut b) = ([0; 8192], [0; 8192]);
                loop {
                    let n = source.read(&mut a)?;
                    if n == 0 {
                        break;
                    }
                    existing.read_exact(&mut b[..n])?;
                    ensure!(a[..n] == b[..n], "A different font already uses this name.");
                }
                Ok(())
            }
            Err(e) => Err(e.error.into()),
        }
    }
}
impl Fonts {
    pub(super) fn browse(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        self.opening = true;
        self.status = "Reading installed fonts…".into();
        let dir = self.inventory_dir.clone();
        let job = cx
            .background_executor()
            .spawn(async move { catalog::installed(dir.as_deref()) });
        cx.spawn_in(w, async move |v, cx| {
            let result = job.await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.opening = false;
                match result {
                    Ok(families) => {
                        v.download = None;
                        v.families = families;
                        v.search.update(cx, |f, cx| {
                            f.select_match(0..f.text().len(), cx);
                            f.replace_text_in_range(None, "", w, cx);
                        });
                        v.filter.clear();
                        v.filter_rows(cx);
                        v.list.focus.focus(w, cx);
                    }
                    Err(e) => v.status = e,
                }
                cx.notify()
            });
        })
        .detach();
        cx.notify()
    }
    pub(super) fn open_file(&mut self, path: PathBuf, w: &mut Window, cx: &mut Context<Self>) {
        self.opening = true;
        self.status = "Opening font…".into();
        cx.notify();
        let job = cx
            .background_executor()
            .spawn(async move { Download::open(&path) });
        cx.spawn_in(w, async move |v, cx| {
            let result = job.await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.opening = false;
                match result {
                    Ok((file, families)) => {
                        v.download = Some(Arc::new(file));
                        v.did_install = false;
                        v.families = families;
                        v.search.update(cx, |f, cx| {
                            f.select_match(0..f.text().len(), cx);
                            f.replace_text_in_range(None, "", w, cx);
                        });
                        v.filter.clear();
                        v.filter_rows(cx);
                        v.list.focus.focus(w, cx);
                    }
                    Err(e) => v.status = format!("Could not open font · {e}"),
                }
                cx.notify()
            });
        })
        .detach();
    }
    pub(super) fn choose(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        self.opening = true;
        let request = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(w, async move |v, cx| {
            let chosen = request.await;
            let _ = v.update_in(cx, |v, w, cx| {
                v.opening = false;
                match chosen {
                    Ok(Ok(Some(mut paths))) if !paths.is_empty() => {
                        v.open_file(paths.remove(0), w, cx)
                    }
                    Ok(Ok(_)) => {}
                    _ => v.status = "Could not open the font chooser.".into(),
                }
                cx.notify()
            });
        })
        .detach();
    }
    pub(super) fn install(&mut self, cx: &mut Context<Self>) {
        if self.opening || self.did_install {
            return;
        }
        let Some(file) = self.download.clone() else {
            return;
        };
        let Some(root) = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|d| d.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        else {
            self.status = "No user font directory available.".into();
            cx.notify();
            return;
        };
        self.opening = true;
        cx.notify();
        let result = cx
            .background_executor()
            .spawn(async move { file.install(&root.join("fonts")) });
        cx.spawn(async move |v, cx| {
            let r = result.await;
            let _ = v.update(cx, |v, cx| {
                v.opening = false;
                match r {
                    Ok(()) => {
                        v.did_install = true;
                        v.status = "Installed for this user.".into();
                    }
                    Err(e) => v.status = format!("Could not install font · {e}"),
                }
                cx.notify()
            });
        })
        .detach();
    }
}
#[cfg(test)]
mod tests {
    use super::{Download, Path, catalog, fs};
    #[test]
    fn install_snapshot_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Survey.ttc");
        fs::copy(Path::new(catalog::FIXTURE).join("Survey.ttc"), &path).unwrap();
        let (font, families) = Download::open(&path).unwrap();
        assert_eq!(families[0].faces.len(), 2);
        fs::write(&path, b"replaced source").unwrap();
        let fonts = dir.path().join("fonts");
        font.install(&fonts).unwrap();
        font.install(&fonts).unwrap();
        let dest = fonts.join("Survey.ttc");
        assert_eq!(
            fs::read(&dest).unwrap(),
            fs::read(font.file.path()).unwrap()
        );
        fs::write(&dest, b"existing").unwrap();
        assert!(font.install(&fonts).is_err());
        assert_eq!(fs::read(dest).unwrap(), b"existing");
    }
}
