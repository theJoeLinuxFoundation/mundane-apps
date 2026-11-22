//! ZIP and tar extraction into a fresh directory, published without replacement.
mod reader;
mod writer;
use anyhow::{Context, Result, bail, ensure};
use reader::Reader;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    os::{
        fd::AsRawFd,
        unix::fs::{PermissionsExt, symlink},
    },
    path::{Component, Path, PathBuf},
};
pub use writer::{Stamp, edit, inspect};

// libarchive converts pathnames via the calling thread's locale. Rust leaves the
// process in the C locale, including when LANG names a UTF-8 locale.
struct PathLocale {
    previous: libc::locale_t,
    current: libc::locale_t,
}
impl PathLocale {
    fn enter() -> Result<Self> {
        unsafe {
            let current = libc::newlocale(
                libc::LC_CTYPE_MASK,
                c"C.UTF-8".as_ptr(),
                std::ptr::null_mut(),
            );
            ensure!(!current.is_null(), "UTF-8 filename locale unavailable.");
            let previous = libc::uselocale(current);
            if previous.is_null() {
                libc::freelocale(current);
                bail!("Could not select UTF-8 filename locale.")
            }
            Ok(Self { previous, current })
        }
    }
}
impl Drop for PathLocale {
    fn drop(&mut self) {
        unsafe {
            libc::uselocale(self.previous);
            libc::freelocale(self.current);
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    File,
    Directory,
    SymLink(PathBuf),
    HardLink(PathBuf),
}
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: Kind,
    pub size: u64,
}
fn clean_entry(path: &Path) -> Result<PathBuf> {
    let mut result = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(v) => result.push(v),
            Component::CurDir => {}
            _ => bail!("Unsafe archive path {}", path.display()),
        }
    }
    Ok(result)
}
fn validate_link(entry: &Path, target: &Path) -> Result<()> {
    let mut depth = entry.components().count().saturating_sub(1);
    ensure!(!target.as_os_str().is_empty(), "Empty link target");
    for c in target.components() {
        match c {
            Component::Normal(_) => depth += 1,
            Component::ParentDir if depth > 0 => depth -= 1,
            Component::CurDir => {}
            _ => bail!("Unsafe link {} → {}", entry.display(), target.display()),
        }
    }
    Ok(())
}
pub fn list(path: &Path) -> Result<Vec<Entry>> {
    let mut reader = Reader::open(path)?;
    let mut nodes = BTreeMap::new();
    while let Some(entry) = reader.next()? {
        if entry.path.as_os_str().is_empty() {
            continue;
        }
        for parent in entry
            .path
            .ancestors()
            .skip(1)
            .take_while(|p| !p.as_os_str().is_empty())
        {
            nodes.entry(parent.to_owned()).or_insert_with(|| Entry {
                path: parent.to_owned(),
                kind: Kind::Directory,
                size: 0,
            });
        }
        if let Some(old) = nodes.insert(entry.path.clone(), entry.clone()) {
            ensure!(
                old.kind == Kind::Directory && entry.kind == Kind::Directory,
                "Conflicting entries at {}",
                entry.path.display()
            );
        }
    }
    for e in nodes.values() {
        if let Some(parent) = e.path.parent().and_then(|p| nodes.get(p)) {
            ensure!(
                parent.kind == Kind::Directory,
                "Entry under a non-directory at {}",
                e.path.display()
            )
        }
    }
    Ok(nodes.into_values().collect())
}
pub fn extract(
    archive: &Path,
    destination: &Path,
    selection: &BTreeSet<PathBuf>,
) -> Result<PathBuf> {
    let dest = File::open(destination)?;
    ensure!(dest.metadata()?.is_dir(), "Choose a destination folder.");
    let base = PathBuf::from(format!("/proc/self/fd/{}", dest.as_raw_fd()));
    let stage = tempfile::Builder::new()
        .prefix(".mundane-extract-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(&base)?;
    let stage_dir = File::open(stage.path())?;
    let root = PathBuf::from(format!("/proc/self/fd/{}", stage_dir.as_raw_fd()));
    let mut reader = Reader::open(archive)?;
    let mut links = Vec::new();
    let mut extracted = false;
    while let Some(entry) = reader.next()? {
        if entry.path.as_os_str().is_empty()
            || (!selection.is_empty() && !entry.path.ancestors().any(|p| selection.contains(p)))
        {
            continue;
        }
        let output = root.join(&entry.path);
        extracted = true;
        // No archive-supplied links exist in the private tree while writing data.
        fs::create_dir_all(output.parent().context("Missing entry parent")?)?;
        match entry.kind {
            Kind::Directory => fs::create_dir_all(output)?,
            Kind::File => reader.copy(&mut File::create_new(output)?)?,
            _ => links.push(entry),
        }
    }
    ensure!(extracted, "The selection contains no extractable entries.");
    for entry in links {
        let output = root.join(&entry.path);
        match entry.kind {
            Kind::SymLink(target) => {
                let actual = output
                    .parent()
                    .unwrap()
                    .join(&target)
                    .canonicalize()
                    .with_context(|| {
                        format!("Missing selected link target for {}", entry.path.display())
                    })?;
                ensure!(
                    actual.starts_with(root.canonicalize()?),
                    "Link target escapes the extraction folder."
                );
                symlink(target, output)?;
            }
            Kind::HardLink(target) => {
                let actual = root.join(target).canonicalize()?;
                ensure!(
                    actual.starts_with(root.canonicalize()?) && actual.is_file(),
                    "Invalid hard link target."
                );
                fs::hard_link(actual, output)?;
            }
            _ => unreachable!(),
        }
    }
    let stem = archive.file_stem().context("Missing archive name")?;
    let mut name = stem.to_owned();
    name.push(" (extracted)");
    // One filesystem operation publishes the complete tree. Existing files,
    // directories and symlinks at the chosen name all cause EEXIST.
    rustix::fs::renameat_with(
        &dest,
        stage.path().file_name().unwrap(),
        &dest,
        &name,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    let _ = stage.keep();
    Ok(destination.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    #[test]
    fn confinement_and_link_extraction() {
        for bad in ["../outside", "/absolute", "ok/../../outside"] {
            assert!(clean_entry(Path::new(bad)).is_err());
            assert!(validate_link(Path::new("link"), Path::new(bad)).is_err())
        }
        assert!(validate_link(Path::new("dir/link"), Path::new("../file")).is_ok());
        let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let archive = project.join("tests/fixtures/archive/Survey links.tar");
        let temp = tempfile::tempdir_in(project.join("target")).unwrap();
        let all = BTreeSet::new();
        for name in [
            "Outside path.tar",
            "Outside link.tar",
            "Outside hardlink.tar",
        ] {
            let unsafe_archive = archive.with_file_name(name);
            assert!(list(&unsafe_archive).is_err());
            assert!(extract(&unsafe_archive, temp.path(), &all).is_err());
            assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        }
        let subset = BTreeSet::from([PathBuf::from("Notes/current")]);
        assert!(extract(&archive, temp.path(), &subset).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        let out = extract(&archive, temp.path(), &all).unwrap();
        assert_eq!(
            fs::read(out.join("Notes/current")).unwrap(),
            b"East channel\n"
        );
        assert_eq!(
            fs::metadata(out.join("copy")).unwrap().ino(),
            fs::metadata(out.join("Charts/channel.txt")).unwrap().ino()
        );
        assert!(extract(&archive, temp.path(), &all).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}
