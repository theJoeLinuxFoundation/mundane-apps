//! Stage replacements in private directories. The reviewed destination remains
//! available until the complete incoming item can be exchanged with it.
use super::{Transfer, checked_command, publish};
use rustix::fs::{CWD, RenameFlags, renameat_with};
use std::{
    fs,
    io::{self, ErrorKind},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::SystemTime,
};
use tempfile::TempDir;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemInfo {
    dev: u64,
    inode: u64,
    mode: u32,
    pub bytes: u64,
    pub modified: SystemTime,
    change: (i64, i64),
}
impl ItemInfo {
    fn read(path: &Path) -> io::Result<Self> {
        let m = fs::symlink_metadata(path)?;
        Ok(Self {
            dev: m.dev(),
            inode: m.ino(),
            mode: m.mode(),
            bytes: m.len(),
            modified: m.modified()?,
            change: (m.ctime(), m.ctime_nsec()),
        })
    }
    pub fn description(&self) -> String {
        let kind = match self.mode & 0o170000 {
            0o120000 => "Link",
            0o040000 => "Folder entry",
            0o100000 => "File",
            _ => "Item",
        };
        let date = jiff::Timestamp::try_from(self.modified)
            .map(|t| t.strftime("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|_| "Unknown date".into());
        format!("{kind} · {} B · Modified {date}", self.bytes)
    }
    // A rename changes ctime. Compare the entry we actually moved or displaced.
    fn matches_moved(&self, path: &Path) -> io::Result<bool> {
        let mut found = Self::read(path)?;
        found.change = self.change;
        Ok(*self == found)
    }
}
#[derive(Clone)]
pub struct Conflict {
    pub incoming: ItemInfo,
    pub existing: ItemInfo,
    pub reason: Option<&'static str>,
}
impl Conflict {
    pub fn inspect(src: &Path, dst: &Path) -> io::Result<Self> {
        let incoming = ItemInfo::read(src)?;
        let existing = ItemInfo::read(dst)?;
        let entry_path = |p: &Path| -> io::Result<PathBuf> {
            Ok(p.parent()
                .ok_or(ErrorKind::InvalidInput)?
                .canonicalize()?
                .join(p.file_name().ok_or(ErrorKind::InvalidInput)?))
        };
        let a = entry_path(src)?;
        let b = entry_path(dst)?;
        let reason = if incoming.dev == existing.dev && incoming.inode == existing.inode {
            Some("Both names refer to the same item.")
        } else if a.starts_with(&b) || b.starts_with(&a) {
            Some("One folder contains the other item.")
        } else {
            None
        };
        Ok(Self {
            incoming,
            existing,
            reason,
        })
    }
}
fn temp(parent: &Path) -> io::Result<TempDir> {
    tempfile::Builder::new()
        .prefix(".mundane-replace-")
        .tempdir_in(parent)
}
fn changed() -> io::Error {
    io::Error::other("An item changed. Paste again to review it.")
}
fn exchange(a: &Path, b: &Path) -> io::Result<()> {
    renameat_with(CWD, a, CWD, b, RenameFlags::EXCHANGE).map_err(Into::into)
}
pub fn replace(src: &Path, dst: &Path, how: Transfer, review: &Conflict) -> io::Result<()> {
    let current = Conflict::inspect(src, dst)?;
    if let Some(reason) = current.reason {
        return Err(io::Error::other(reason));
    }
    if current.incoming != review.incoming || current.existing != review.existing {
        return Err(changed());
    }
    let dest_stage = temp(dst.parent().ok_or(ErrorKind::InvalidInput)?)?;
    let dest_item = dest_stage.path().join("item");
    let same_mount = review.incoming.dev == fs::metadata(dest_stage.path())?.dev();
    let held = if how == Transfer::Move {
        let dir = temp(src.parent().ok_or(ErrorKind::InvalidInput)?)?;
        publish(src, &dir.path().join("item"))?;
        Some(dir)
    } else {
        None
    };
    let original = held.as_ref().map(|d| d.path().join("item"));
    let source = original.as_deref().unwrap_or(src);
    let direct = held.is_some() && same_mount;
    let payload = if direct { source } else { &dest_item };
    let mut uncertain = false;
    let result = (|| {
        if !direct {
            checked_command(
                "cp",
                &[
                    "-aT".as_ref(),
                    "--".as_ref(),
                    source.as_os_str(),
                    dest_item.as_os_str(),
                ],
            )?;
        }
        if !review.incoming.matches_moved(source)? || ItemInfo::read(dst)? != review.existing {
            return Err(changed());
        }
        let prepared = ItemInfo::read(payload)?;
        exchange(payload, dst)?;
        // If another process replaced the destination between stat and exchange,
        // put that item back. Never clean up an unrecognised displaced entry.
        if !review.existing.matches_moved(payload).unwrap_or(false) {
            uncertain = true;
            if !prepared.matches_moved(dst)? {
                return Err(changed());
            }
            exchange(payload, dst)?;
            if !prepared.matches_moved(payload)? {
                return Err(changed());
            }
            uncertain = false;
            return Err(changed());
        }
        Ok(())
    })();
    if uncertain {
        let dest_dir = dest_stage.keep();
        let recovery = held.map(TempDir::keep).unwrap_or_else(|| dest_dir.clone());
        return Err(io::Error::other(format!(
            "Replacement interrupted. Inspect {} and {}.",
            recovery.display(),
            dest_dir.display()
        )));
    }
    if let Err(e) = result {
        if let Some(dir) = held
            && let Err(restore) = publish(&dir.path().join("item"), src)
        {
            return Err(io::Error::other(format!(
                "{e}. Could not restore the source ({restore}). It is in {}.",
                dir.keep().display()
            )));
        }
        return Err(e);
    }
    for dir in [Some(dest_stage), held].into_iter().flatten() {
        let path = dir.path().to_owned();
        dir.close().map_err(|e| {
            io::Error::other(format!(
                "Replaced the item. Could not finish cleaning up {}. {e}",
                path.display()
            ))
        })?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    #[test]
    fn copy_move_links_and_stale_reviews() {
        let tmp = super::super::tests::fixture();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::write(&a, "new").unwrap();
        fs::write(&b, "old").unwrap();
        let stale = Conflict::inspect(&a, &b).unwrap();
        fs::write(&b, "edited").unwrap();
        assert!(replace(&a, &b, Transfer::Copy, &stale).is_err());
        assert_eq!(fs::read(&b).unwrap(), b"edited");
        let fresh = Conflict::inspect(&a, &b).unwrap();
        replace(&a, &b, Transfer::Copy, &fresh).unwrap();
        assert_eq!(fs::read(&b).unwrap(), b"new");
        assert!(a.exists());
        fs::remove_file(&b).unwrap();
        symlink(&a, &b).unwrap();
        let reviewed = Conflict::inspect(&a, &b).unwrap();
        replace(&a, &b, Transfer::Move, &reviewed).unwrap();
        assert!(!a.exists());
        assert!(!b.is_symlink());
        assert_eq!(fs::read(&b).unwrap(), b"new");
        assert!(Conflict::inspect(&b, &b).unwrap().reason.is_some())
    }
    #[test]
    fn whole_folders_and_ancestors() {
        let tmp = super::super::tests::fixture();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        for dir in [&a, &b] {
            fs::create_dir(dir).unwrap()
        }
        fs::write(a.join("new"), "a").unwrap();
        fs::write(b.join("old"), "b").unwrap();
        assert!(
            Conflict::inspect(&a, &a.join("new"))
                .unwrap()
                .reason
                .is_some()
        );
        assert!(
            Conflict::inspect(&b.join("old"), &b)
                .unwrap()
                .reason
                .is_some()
        );
        replace(&a, &b, Transfer::Move, &Conflict::inspect(&a, &b).unwrap()).unwrap();
        assert!(!a.exists());
        assert!(b.join("new").exists());
        assert!(!b.join("old").exists())
    }
}
