//! Narrow read-only bindings to the system libarchive. The reader owns its
//! native handle and keeps the underlying descriptor open until after free.
use super::{Entry, Kind, clean_entry, validate_link};
use anyhow::{Context, Result, bail, ensure};
use std::{
    ffi::{CStr, OsStr, c_char, c_int, c_void},
    fs::File,
    io::Write,
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::{Path, PathBuf},
    ptr::NonNull,
};

#[link(name = "archive")]
unsafe extern "C" {
    fn archive_read_new() -> *mut c_void;
    fn archive_read_support_filter_xz(a: *mut c_void) -> c_int;
    fn archive_read_support_filter_gzip(a: *mut c_void) -> c_int;
    fn archive_read_support_format_tar(a: *mut c_void) -> c_int;
    fn archive_read_support_format_zip(a: *mut c_void) -> c_int;
    fn archive_read_open_fd(a: *mut c_void, fd: c_int, block: usize) -> c_int;
    fn archive_read_next_header(a: *mut c_void, e: *mut *mut c_void) -> c_int;
    fn archive_read_data(a: *mut c_void, b: *mut c_void, len: usize) -> isize;
    fn archive_error_string(a: *mut c_void) -> *const c_char;
    fn archive_read_free(a: *mut c_void) -> c_int;
    fn archive_entry_pathname(e: *mut c_void) -> *const c_char;
    fn archive_entry_symlink(e: *mut c_void) -> *const c_char;
    fn archive_entry_hardlink(e: *mut c_void) -> *const c_char;
    fn archive_entry_filetype(e: *mut c_void) -> u32;
    fn archive_entry_size(e: *mut c_void) -> i64;
}

pub(super) struct Reader {
    handle: NonNull<c_void>,
    _file: File,
    _locale: super::PathLocale,
    pub(super) header: *mut c_void,
}
impl Reader {
    pub fn open(path: &Path) -> Result<Self> {
        let locale = super::PathLocale::enter()?;
        let file = File::options()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open(path)?;
        ensure!(
            file.metadata()?.is_file(),
            "Choose a regular ZIP or tar file."
        );
        // All calls use the fresh, exclusively owned reader. A warning from the
        // gzip setup can mean an external decompressor, so require ARCHIVE_OK.
        unsafe {
            let handle =
                NonNull::new(archive_read_new()).context("Could not allocate archive reader")?;
            let reader = Self {
                handle,
                _file: file,
                _locale: locale,
                header: std::ptr::null_mut(),
            };
            let a = handle.as_ptr();
            reader.check(archive_read_support_filter_gzip(a))?;
            reader.check(archive_read_support_filter_xz(a))?;
            reader.check(archive_read_support_format_tar(a))?;
            reader.check(archive_read_support_format_zip(a))?;
            reader.check(archive_read_open_fd(a, reader._file.as_raw_fd(), 65536))?;
            Ok(reader)
        }
    }
    fn check(&self, code: c_int) -> Result<()> {
        if code != 0 {
            let p = unsafe { archive_error_string(self.handle.as_ptr()) };
            if p.is_null() {
                bail!("libarchive returned {code}")
            }
            bail!("{}", unsafe { CStr::from_ptr(p) }.to_string_lossy())
        }
        Ok(())
    }
    pub fn next(&mut self) -> Result<Option<Entry>> {
        // libarchive owns header strings until the next call. Copy them here.
        unsafe {
            let mut e = std::ptr::null_mut();
            let r = archive_read_next_header(self.handle.as_ptr(), &mut e);
            if r == 1 {
                return Ok(None);
            }
            self.check(r)?;
            ensure!(!e.is_null(), "Missing archive header.");
            self.header = e;
            let path = |p: *const c_char| {
                (!p.is_null())
                    .then(|| PathBuf::from(OsStr::from_bytes(CStr::from_ptr(p).to_bytes())))
            };
            let name =
                clean_entry(&path(archive_entry_pathname(e)).context("Missing entry name")?)?;
            let kind = if let Some(link) = path(archive_entry_hardlink(e)) {
                Kind::HardLink(clean_entry(&link)?)
            } else if let Some(link) = path(archive_entry_symlink(e)) {
                validate_link(&name, &link)?;
                Kind::SymLink(link)
            } else {
                match archive_entry_filetype(e) {
                    0o040000 => Kind::Directory,
                    0o100000 => Kind::File,
                    _ => bail!("Unsupported entry type for {}", name.display()),
                }
            };
            ensure!(
                !name.as_os_str().is_empty() || kind == Kind::Directory,
                "Empty entry name"
            );
            Ok(Some(Entry {
                path: name,
                kind,
                size: archive_entry_size(e).max(0) as u64,
            }))
        }
    }
    pub fn copy(&mut self, out: &mut impl Write) -> Result<()> {
        let mut buf = [0u8; 65536];
        loop {
            let n = unsafe {
                archive_read_data(self.handle.as_ptr(), buf.as_mut_ptr().cast(), buf.len())
            };
            if n < 0 {
                self.check(n as i32)?
            }
            if n == 0 {
                return Ok(());
            }
            out.write_all(&buf[..n as usize])?;
        }
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        unsafe {
            archive_read_free(self.handle.as_ptr());
        }
    }
}
