//! Local file operations. Call these on a background executor.
mod navigation;
pub use navigation::{Place, SearchResults, places, search, toggle_bookmark};
mod replace;
mod thumbnail;
pub use replace::{Conflict, replace};
use rustix::fs::{CWD, RenameFlags, renameat_with};
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, ErrorKind},
    os::unix::fs::OpenOptionsExt,
    path::{Component, Path, PathBuf},
    process::Command,
};
pub use thumbnail::preview;

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: OsString,
    pub folder: bool,
    pub symlink: bool,
    pub mime: String,
    pub bytes: u64,
}

pub fn list(directory: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(directory)? {
        let item = item?;
        let path = item.path();
        let metadata = match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            result => result?,
        };
        let symlink = metadata.is_symlink();
        let folder = metadata.is_dir() || (symlink && path.is_dir());
        let mime = if folder {
            "inode/directory"
        } else {
            mime_guess::from_path(&path)
                .first_raw()
                .unwrap_or("application/octet-stream")
        };
        entries.push(Entry {
            name: item.file_name(),
            path,
            folder,
            symlink,
            mime: mime.into(),
            bytes: metadata.len(),
        });
    }
    entries.sort_by(|a, b| b.folder.cmp(&a.folder).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

pub fn icon(entry: &Entry) -> Option<PathBuf> {
    let generic = match entry.mime.split('/').next()? {
        "image" => "image-x-generic",
        "text" => "text-x-generic",
        "audio" => "audio-x-generic",
        "video" => "video-x-generic",
        _ if entry.folder => "folder",
        _ => "application-x-generic",
    };
    static DESKTOP_THEME: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let theme = DESKTOP_THEME.get_or_init(freedesktop_icons::default_theme_gtk);
    for theme in theme.as_deref().into_iter().chain(["Papirus", "Adwaita"]) {
        for name in [&entry.mime.replace('/', "-"), generic] {
            if let Some(path) = freedesktop_icons::lookup(name)
                .with_theme(theme)
                .with_size(32)
                .with_cache()
                .find()
            {
                return Some(path);
            }
        }
    }
    None
}

fn checked_command(program: &str, args: &[&OsStr]) -> io::Result<()> {
    let result = Command::new(program).args(args).output()?;
    if result.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&result.stderr).trim().to_owned(),
        ))
    }
}

fn publish(source: &Path, destination: &Path) -> io::Result<()> {
    renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE).map_err(Into::into)
}

pub fn rename(source: &Path, name: &OsStr) -> io::Result<PathBuf> {
    if !matches!(Path::new(name).components().next(), Some(Component::Normal(part)) if part == name)
    {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Enter a single file name.",
        ));
    }
    let dest = source.with_file_name(name);
    if source != dest {
        publish(source, &dest)?;
    }
    Ok(dest)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    Copy,
    Move,
}

pub fn transfer(source: &Path, dest: &Path, operation: Transfer) -> io::Result<()> {
    // A dangling destination symlink also occupies its name.
    match fs::symlink_metadata(dest) {
        Ok(_) => return Err(ErrorKind::AlreadyExists.into()),
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let parent = dest.parent().ok_or(ErrorKind::InvalidInput)?;
    if fs::symlink_metadata(source)?.is_dir()
        && parent.canonicalize()?.starts_with(source.canonicalize()?)
    {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Choose a destination outside the source folder.",
        ));
    }
    if operation == Transfer::Move {
        match publish(source, dest) {
            Err(err) if err.kind() == ErrorKind::CrossesDevices => {
                // Coreutils handles copying metadata and removing the source across mounts.
                let result = checked_command(
                    "mv",
                    &[
                        "--update=none-fail".as_ref(),
                        "-T".as_ref(),
                        "--".as_ref(),
                        source.as_os_str(),
                        dest.as_os_str(),
                    ],
                );
                return match result {
                    Err(_)
                        if fs::symlink_metadata(source).is_ok()
                            && fs::symlink_metadata(dest).is_ok() =>
                    {
                        Err(io::Error::other(
                            "Move incomplete. Inspect the source and destination before retrying.",
                        ))
                    }
                    other => other,
                };
            }
            result => return result,
        }
    }
    // Stage on the destination mount. Only a complete tree can acquire the final name.
    let stage = tempfile::Builder::new()
        .prefix(".mundane-copy-")
        .tempdir_in(parent)?;
    let payload = stage.path().join("item");
    checked_command(
        "cp",
        &[
            "-aT".as_ref(),
            "--".as_ref(),
            source.as_os_str(),
            payload.as_os_str(),
        ],
    )?;
    publish(&payload, dest)
}

pub fn keep_both(source: &Path, dest: &Path, operation: Transfer) -> io::Result<PathBuf> {
    let stem = dest.file_stem().ok_or(ErrorKind::InvalidInput)?;
    for n in 1_u32.. {
        let mut name = stem.to_os_string();
        name.push(if n == 1 {
            " (copy)".into()
        } else {
            format!(" (copy {n})")
        });
        if let Some(ext) = dest.extension() {
            name.push(".");
            name.push(ext);
        }
        let alternate = dest.with_file_name(name);
        match transfer(source, &alternate, operation) {
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            result => return result.map(|()| alternate),
        }
    }
    Err(io::Error::other("No available copy name."))
}

pub fn trash(path: &Path) -> io::Result<()> {
    checked_command("gio", &["trash".as_ref(), "--".as_ref(), path.as_os_str()])
}

fn portal_unavailable(error: &ashpd::Error) -> bool {
    use ashpd::{Error, PortalError, zbus::fdo};
    match error {
        Error::PortalNotFound(_) => true,
        Error::Zbus(bus) | Error::Portal(PortalError::ZBus(bus)) => matches!(
            fdo::Error::from(bus.clone()),
            fdo::Error::ServiceUnknown(_)
                | fdo::Error::NameHasNoOwner(_)
                | fdo::Error::UnknownInterface(_)
                | fdo::Error::UnknownMethod(_)
        ),
        _ => false,
    }
}

pub async fn open(path: &Path) -> Result<(), String> {
    use ashpd::{
        Error,
        desktop::{ResponseError, open_uri::OpenFileRequest},
    };
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Only regular files can be opened.".into());
    }
    match OpenFileRequest::default()
        .send_file(&file)
        .await
        .and_then(|request| request.response())
    {
        Ok(()) | Err(Error::Response(ResponseError::Cancelled)) => Ok(()),
        // Without a portal desktop service, use the installed GIO MIME association.
        Err(error) if portal_unavailable(&error) => {
            checked_command("gio", &["open".as_ref(), path.as_os_str()]).map_err(|e| e.to_string())
        }
        Err(err) => Err(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    pub(super) fn fixture() -> tempfile::TempDir {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        tempfile::tempdir_in(parent).unwrap()
    }
    #[test]
    fn collisions_names_and_dangling_links() {
        let tmp = fixture();
        let dir = tmp.path();
        let link = dir.join("broken");
        symlink("missing", &link).unwrap();
        assert!(list(dir).unwrap()[0].symlink);
        let source = dir.join("report.txt");
        fs::write(&source, "original").unwrap();
        for op in [Transfer::Copy, Transfer::Move] {
            assert_eq!(
                transfer(&source, &link, op).unwrap_err().kind(),
                ErrorKind::AlreadyExists
            );
        }
        assert_eq!(
            rename(&source, OsStr::new("broken")).unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
        for name in ["", ".", "..", "/escape", "../escape", "folder/"] {
            assert_eq!(
                rename(&source, OsStr::new(name)).unwrap_err().kind(),
                ErrorKind::InvalidInput
            );
        }
        let copy = keep_both(&source, &source, Transfer::Copy).unwrap();
        assert_eq!(fs::read(copy).unwrap(), b"original");
        let moved = dir.join("renamed");
        transfer(&link, &moved, Transfer::Move).unwrap();
        transfer(&moved, &link, Transfer::Copy).unwrap();
        assert_eq!(fs::read_link(link).unwrap(), Path::new("missing"));
        assert!(dir.join("renamed").is_symlink());
    }
    #[test]
    fn directory_copies_are_complete_and_never_recursive_into_themselves() {
        let tmp = fixture();
        let src = tmp.path().join("source");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("item"), b"data").unwrap();
        symlink("item", src.join("link")).unwrap();
        let dest = tmp.path().join("dest");
        transfer(&src, &dest, Transfer::Copy).unwrap();
        assert_eq!(fs::read(dest.join("item")).unwrap(), b"data");
        assert_eq!(fs::read_link(dest.join("link")).unwrap(), Path::new("item"));
        assert!(transfer(&src, &src.join("inside"), Transfer::Copy).is_err());
        assert!(!src.join("inside").exists());
    }
    #[test]
    fn denied_writes_leave_source_and_destination_intact() {
        let tmp = fixture();
        let src = tmp.path().join("item");
        fs::write(&src, "original").unwrap();
        let locked = tmp.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
        let copied = transfer(&src, &locked.join("new"), Transfer::Copy);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            copied.unwrap_err().kind(),
            ErrorKind::PermissionDenied,
            "run permission tests as an unprivileged desktop user"
        );
        assert_eq!(fs::read(&src).unwrap(), b"original");
        assert!(list(&locked).unwrap().is_empty());
    }
}
