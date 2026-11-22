use super::{Entry, list};
use std::{
    collections::{HashSet, VecDeque},
    ffi::OsString,
    fs,
    io::{self, Write},
    os::unix::{ffi::OsStringExt, fs::MetadataExt},
    path::{Path, PathBuf},
};
use url::Url;

pub struct Place {
    pub path: PathBuf,
    pub name: String,
    pub group: &'static str,
}

fn bookmark_path() -> io::Result<PathBuf> {
    dirs::config_dir()
        .map(|p| p.join("gtk-3.0/bookmarks"))
        .ok_or_else(|| io::Error::other("No configuration directory"))
}
fn bookmark(line: &str) -> Option<(PathBuf, Option<&str>)> {
    let (uri, label) = line
        .split_once(' ')
        .map_or((line, None), |(u, l)| (u, Some(l)));
    Some((Url::parse(uri).ok()?.to_file_path().ok()?, label))
}

fn edit_bookmarks(file: &Path, path: &Path) -> io::Result<bool> {
    let parent = file.parent().ok_or(io::ErrorKind::InvalidInput)?;
    fs::create_dir_all(parent)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(parent.join(".mundane-bookmarks.lock"))?;
    lock.lock()?;
    let old = match fs::read_to_string(file) {
        Ok(data) => data,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let mut new = tempfile::NamedTempFile::new_in(parent)?;
    let mut removed = false;
    for line in old.lines() {
        if bookmark(line).is_some_and(|(p, _)| p == path) {
            removed = true;
        } else {
            writeln!(new, "{line}")?;
        }
    }
    if !removed {
        let uri = Url::from_file_path(path).map_err(|_| io::ErrorKind::InvalidInput)?;
        writeln!(new, "{uri}")?;
    }
    new.persist(file)?;
    Ok(!removed)
}
pub fn toggle_bookmark(path: &Path) -> io::Result<bool> {
    edit_bookmarks(&bookmark_path()?, path)
}

fn mount_path(value: &[u8]) -> PathBuf {
    let mut out = Vec::new();
    let mut bytes = value;
    while let [b, tail @ ..] = bytes {
        if *b == b'\\'
            && let [a @ b'0'..=b'3', b @ b'0'..=b'7', c @ b'0'..=b'7', rest @ ..] = tail
        {
            out.push((a - b'0') * 64 + (b - b'0') * 8 + (c - b'0'));
            bytes = rest;
        } else {
            out.push(*b);
            bytes = tail;
        }
    }
    OsString::from_vec(out).into()
}

pub fn places() -> (Vec<Place>, HashSet<PathBuf>) {
    let mut bookmarks = HashSet::new();
    let mut rows = vec![];
    let mut seen = HashSet::new();
    let mut add = |p: PathBuf, name: Option<&str>, group| {
        if seen.insert(p.clone()) {
            rows.push(Place {
                name: name.map(str::to_owned).unwrap_or_else(|| {
                    p.file_name()
                        .unwrap_or(p.as_os_str())
                        .to_string_lossy()
                        .into()
                }),
                path: p,
                group,
            })
        }
    };
    if let Some(home) = dirs::home_dir() {
        add(home, Some("Home"), "Places");
    }
    for path in [
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
        dirs::picture_dir(),
        dirs::audio_dir(),
        dirs::video_dir(),
    ]
    .into_iter()
    .flatten()
    {
        if path.is_dir() {
            add(path, None, "Places");
        }
    }
    add(PathBuf::from("/"), Some("File system"), "Places");
    if let Ok(file) = bookmark_path()
        && let Ok(lines) = fs::read_to_string(file)
    {
        for (path, label) in lines.lines().filter_map(bookmark) {
            bookmarks.insert(path.clone());
            add(path, label, "Bookmarks");
        }
    }
    if let Ok(mounts) = fs::read("/proc/self/mountinfo") {
        for row in mounts.split(|b| *b == b'\n') {
            if let Some(field) = row.split(|b| *b == b' ').nth(4) {
                let path = mount_path(field);
                if ["/mnt", "/media", "/run/media"]
                    .iter()
                    .any(|root| path.starts_with(root))
                {
                    add(path, None, "Mounted volumes");
                }
            }
        }
    }
    (rows, bookmarks)
}

pub struct SearchResults {
    pub entries: Vec<Entry>,
    pub skipped: usize,
    pub limited: bool,
}
pub fn search(
    root: &Path,
    query: &str,
    hidden: bool,
    cancelled: impl Fn() -> bool,
) -> io::Result<SearchResults> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Err(io::Error::other("Enter part of a file name."));
    }
    let mut result = SearchResults {
        entries: vec![],
        skipped: 0,
        limited: false,
    };
    let device = fs::metadata(root)?.dev();
    let mut todo = VecDeque::from([root.to_owned()]);
    let mut seen = HashSet::new();
    while let Some(dir) = todo.pop_front() {
        if cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let listing = fs::metadata(&dir).and_then(|m| {
            if m.dev() == device && seen.insert((m.dev(), m.ino())) {
                list(&dir)
            } else {
                Ok(vec![])
            }
        });
        let entries = match listing {
            Ok(v) => v,
            Err(e) if dir == root => return Err(e),
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        for entry in entries {
            if !hidden && entry.name.as_encoded_bytes().starts_with(b".") {
                continue;
            }
            if entry.folder && !entry.symlink {
                todo.push_back(entry.path.clone());
            }
            if entry.name.to_string_lossy().to_lowercase().contains(&query) {
                if result.entries.len() == 1000 {
                    result.limited = true;
                    return Ok(result);
                }
                result.entries.push(entry)
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_search_hidden_links_and_bookmark_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let child = root.join("Field notes");
        fs::create_dir(&child).unwrap();
        fs::write(child.join("NOTE.txt"), "x").unwrap();
        fs::write(root.join(".note"), "x").unwrap();
        std::os::unix::fs::symlink(root, child.join("loop")).unwrap();
        assert_eq!(
            search(root, "NoTe", false, || false).unwrap().entries.len(),
            2
        );
        assert_eq!(
            search(root, "note", true, || false).unwrap().entries.len(),
            3
        );
        assert!(search(root, "note", true, || true).is_err());
        let f = root.join("bookmarks");
        fs::write(&f, "sftp://host/folder Remote\n").unwrap();
        assert!(edit_bookmarks(&f, &child).unwrap());
        assert!(fs::read_to_string(&f).unwrap().contains("Field%20notes"));
        assert!(!edit_bookmarks(&f, &child).unwrap());
        assert_eq!(
            fs::read_to_string(f).unwrap(),
            "sftp://host/folder Remote\n"
        );
        assert_eq!(
            mount_path(br"/media/Field\040notes"),
            Path::new("/media/Field notes")
        );
    }
}
