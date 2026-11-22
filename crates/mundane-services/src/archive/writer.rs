//! Stream retained libarchive entries into a sibling temporary file before publishing.
use super::*;
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    io::{self, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    ptr::NonNull,
};
unsafe extern "C" {
    fn archive_write_new() -> *mut c_void;
    fn archive_write_set_format_zip(a: *mut c_void) -> c_int;
    fn archive_write_set_format_pax_restricted(a: *mut c_void) -> c_int;
    fn archive_write_add_filter_xz(a: *mut c_void) -> c_int;
    fn archive_write_add_filter_gzip(a: *mut c_void) -> c_int;
    fn archive_write_open_fd(a: *mut c_void, fd: c_int) -> c_int;
    fn archive_write_header(a: *mut c_void, e: *mut c_void) -> c_int;
    fn archive_write_data(a: *mut c_void, data: *const c_void, len: usize) -> isize;
    fn archive_write_close(a: *mut c_void) -> c_int;
    fn archive_write_free(a: *mut c_void) -> c_int;
    fn archive_entry_new() -> *mut c_void;
    fn archive_entry_free(e: *mut c_void);
    fn archive_entry_set_pathname(e: *mut c_void, path: *const c_char);
    fn archive_entry_set_mode(e: *mut c_void, mode: u32);
    fn archive_entry_set_size(e: *mut c_void, size: i64);
    fn archive_entry_set_mtime(e: *mut c_void, sec: i64, nsec: std::ffi::c_long);
    fn archive_entry_sparse_count(e: *mut c_void) -> c_int;
    fn archive_entry_is_encrypted(e: *mut c_void) -> c_int;
    fn archive_error_string(a: *mut c_void) -> *const c_char;
}
struct Writer(NonNull<c_void>);
impl Drop for Writer {
    fn drop(&mut self) {
        unsafe {
            archive_write_free(self.0.as_ptr());
        }
    }
}
impl Writer {
    fn new(path: &Path, file: &File) -> Result<Self> {
        let name = path
            .file_name()
            .context("Missing archive name")?
            .to_string_lossy()
            .to_ascii_lowercase();
        unsafe {
            let w = Self(
                NonNull::new(archive_write_new()).context("Could not allocate archive writer")?,
            );
            let a = w.0.as_ptr();
            if name.ends_with(".zip") {
                w.check(archive_write_set_format_zip(a))?
            } else {
                w.check(archive_write_set_format_pax_restricted(a))?;
                if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
                    w.check(archive_write_add_filter_gzip(a))?
                } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
                    w.check(archive_write_add_filter_xz(a))?
                } else {
                    ensure!(
                        name.ends_with(".tar"),
                        "Choose a .zip, .tar, .tar.gz or .tar.xz name."
                    )
                }
            }
            w.check(archive_write_open_fd(a, file.as_raw_fd()))?;
            Ok(w)
        }
    }
    fn check(&self, code: c_int) -> Result<()> {
        if code != 0 {
            let p = unsafe { archive_error_string(self.0.as_ptr()) };
            if !p.is_null() {
                bail!("{}", unsafe { CStr::from_ptr(p) }.to_string_lossy())
            }
            bail!("libarchive returned {code}");
        }
        Ok(())
    }
    fn add(&mut self, path: &Path, names: &mut BTreeSet<PathBuf>, name: &Path) -> Result<()> {
        ensure!(
            names.insert(name.to_owned()),
            "Entry already exists: {}",
            name.display()
        );
        let mut input = File::options()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(path)?;
        let meta = input.metadata()?;
        ensure!(
            meta.is_file() || meta.is_dir(),
            "Choose regular files or folders."
        );
        let c_name = CString::new(name.as_os_str().as_bytes())?;
        let size = if meta.is_dir() {
            0
        } else {
            i64::try_from(meta.len())?
        };
        unsafe {
            let e = NonNull::new(archive_entry_new())
                .context("Could not allocate archive entry")?
                .as_ptr();
            archive_entry_set_pathname(e, c_name.as_ptr());
            archive_entry_set_mode(
                e,
                (if meta.is_dir() { 0o040000 } else { 0o100000 }) | (meta.mode() & 0o777),
            );
            archive_entry_set_mtime(e, meta.mtime(), meta.mtime_nsec() as _);
            archive_entry_set_size(e, size);
            let result = archive_write_header(self.0.as_ptr(), e);
            archive_entry_free(e);
            self.check(result)?;
        }
        if meta.is_dir() {
            let dir = format!("/proc/self/fd/{}", input.as_raw_fd());
            let mut children = fs::read_dir(&dir)?
                .map(|e| e.map(|e| e.file_name()))
                .collect::<io::Result<Vec<_>>>()?;
            children.sort();
            for child in children {
                self.add(&Path::new(&dir).join(&child), names, &name.join(child))?
            }
        } else {
            ensure!(
                io::copy(&mut input, self)? == meta.len(),
                "Source file size changed."
            );
        }
        Ok(())
    }
}
impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = unsafe { archive_write_data(self.0.as_ptr(), buf.as_ptr().cast(), buf.len()) };
        usize::try_from(n).map_err(|_| io::Error::other(self.check(-1).unwrap_err()))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stamp(u64, u64, u64, i64, i64);
impl Stamp {
    fn read(path: &Path) -> Result<Self> {
        let m = path.metadata()?;
        ensure!(m.is_file(), "Choose a regular archive file to edit.");
        Ok(Self(m.dev(), m.ino(), m.len(), m.ctime(), m.ctime_nsec()))
    }
}
pub fn inspect(path: &Path) -> Result<(Vec<Entry>, Stamp)> {
    let before = Stamp::read(path)?;
    let entries = list(path)?;
    ensure!(
        Stamp::read(path)? == before,
        "Archive changed while opening."
    );
    Ok((entries, before))
}
struct Locked(File);
impl Drop for Locked {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
pub fn edit(
    path: &Path,
    reviewed: Option<Stamp>,
    add: &[PathBuf],
    remove: &BTreeSet<PathBuf>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)?;
    for source in add {
        ensure!(
            !source.is_dir() || !parent.starts_with(source.canonicalize()?),
            "Save the archive outside the folder being added."
        );
        ensure!(
            fs::canonicalize(path).ok().as_ref() != Some(&source.canonicalize()?),
            "An archive cannot contain itself."
        );
    }
    let locked = if let Some(stamp) = reviewed {
        let file = File::options()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(path)?;
        file.try_lock()?;
        let locked = Locked(file);
        ensure!(
            Stamp::read(path)? == stamp,
            "Archive changed. Reopen it before editing."
        );
        Some(locked)
    } else {
        None
    };
    let stage = tempfile::NamedTempFile::new_in(parent)?;
    let _locale = PathLocale::enter()?;
    let mut writer = Writer::new(path, stage.as_file())?;
    let mut names = BTreeSet::new();
    if let Some(file) = locked.as_ref() {
        stage
            .as_file()
            .set_permissions(file.0.metadata()?.permissions())?;
        let mut reader = Reader::open(path)?;
        while let Some(entry) = reader.next()? {
            if entry.path.ancestors().any(|p| remove.contains(p)) {
                continue;
            }
            if let Kind::HardLink(ref dest) = entry.kind {
                ensure!(
                    !dest.ancestors().any(|p| remove.contains(p)),
                    "A retained hard link uses an entry selected for removal."
                );
            }
            names.extend(entry.path.ancestors().map(Path::to_owned));
            unsafe {
                ensure!(
                    archive_entry_sparse_count(reader.header) == 0
                        && archive_entry_is_encrypted(reader.header) <= 0,
                    "Editing encrypted or sparse entries is unsupported."
                );
                writer.check(archive_write_header(writer.0.as_ptr(), reader.header))?;
            }
            reader.copy(&mut writer)?;
        }
    }
    for source in add {
        writer.add(
            source,
            &mut names,
            Path::new(source.file_name().context("Missing input filename")?),
        )?
    }
    unsafe {
        writer.check(archive_write_close(writer.0.as_ptr()))?;
    }
    drop(writer);
    stage.as_file().sync_all()?;
    if let Some(stamp) = reviewed {
        ensure!(
            Stamp::read(path)? == stamp,
            "Archive changed during editing."
        );
        stage.persist(path)?;
    } else {
        stage.persist_noclobber(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_edits_guard_sources_and_reviewed_version() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("Café.txt");
        let second = dir.path().join("Transect.md");
        fs::write(&first, b"east").unwrap();
        fs::write(&second, b"north").unwrap();
        for ext in ["zip", "tar", "tar.gz", "tar.xz"] {
            let target = dir.path().join(format!("Notes.{ext}"));
            edit(
                &target,
                None,
                std::slice::from_ref(&first),
                &BTreeSet::new(),
            )
            .unwrap();
            let (items, stamp) = inspect(&target).unwrap();
            assert_eq!(items[0].path, Path::new("Café.txt"));
            let saved = fs::read(&target).unwrap();
            assert!(
                edit(
                    &target,
                    None,
                    std::slice::from_ref(&second),
                    &BTreeSet::new()
                )
                .is_err()
            );
            assert!(
                edit(
                    &target,
                    Some(stamp),
                    std::slice::from_ref(&first),
                    &BTreeSet::new()
                )
                .is_err()
            );
            assert_eq!(fs::read(&target).unwrap(), saved);
            edit(
                &target,
                Some(stamp),
                std::slice::from_ref(&second),
                &BTreeSet::new(),
            )
            .unwrap();
            assert!(
                edit(
                    &target,
                    Some(stamp),
                    &[],
                    &BTreeSet::from(["Transect.md".into()])
                )
                .is_err()
            );
            let (items, new) = inspect(&target).unwrap();
            assert_eq!(items.len(), 2);
            edit(
                &target,
                Some(new),
                &[],
                &BTreeSet::from(["Café.txt".into()]),
            )
            .unwrap();
            let where_to = tempfile::tempdir().unwrap();
            let out = extract(&target, where_to.path(), &BTreeSet::new()).unwrap();
            assert_eq!(fs::read(out.join("Transect.md")).unwrap(), b"north");
            assert!(!out.join("Café.txt").exists());
        }
    }
}
