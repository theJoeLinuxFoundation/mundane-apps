//! Cancellable per-inode accounting on one filesystem, without following symlinks.
use std::{
    collections::HashSet,
    ffi::OsString,
    fs::{self, File, Metadata},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Default, Clone, Copy)]
pub struct Bytes {
    pub allocated: u64,
    pub apparent: u64,
}
impl Bytes {
    fn add(&mut self, rhs: Self) {
        self.allocated = self.allocated.saturating_add(rhs.allocated);
        self.apparent = self.apparent.saturating_add(rhs.apparent)
    }
    pub fn get(self, apparent: bool) -> u64 {
        if apparent {
            self.apparent
        } else {
            self.allocated
        }
    }
}
pub struct Node {
    pub name: OsString,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub directory: bool,
    pub bytes: Bytes,
    pub note: Option<String>,
    pub partial: bool,
}
pub struct Scan {
    pub root: PathBuf,
    pub nodes: Vec<Node>,
}
struct Frame {
    index: usize,
    fd: File,
    entries: std::vec::IntoIter<OsString>,
}
fn open_dir(path: &Path) -> io::Result<File> {
    File::options()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::DIRECTORY).bits() as i32)
        .open(path)
}
fn check(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "Scan cancelled"))
    } else {
        Ok(())
    }
}
fn fd_path(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}
impl Node {
    fn new(name: OsString, parent: Option<usize>) -> Self {
        Self {
            name,
            parent,
            children: vec![],
            directory: false,
            bytes: Bytes::default(),
            note: None,
            partial: false,
        }
    }
    fn count(&mut self, m: &Metadata) {
        self.directory = m.is_dir();
        self.bytes = Bytes {
            allocated: m.blocks().saturating_mul(512),
            apparent: if m.is_dir() { 0 } else { m.len() },
        };
        if m.is_symlink() {
            self.note = Some("Symbolic link".into())
        }
    }
    fn error(&mut self, e: io::Error) {
        self.note = Some(if e.kind() == io::ErrorKind::PermissionDenied {
            "Unreadable · permission denied".into()
        } else {
            format!("Unreadable · {e}")
        });
        self.partial = true;
    }
}
impl Scan {
    pub fn path(&self, mut index: usize) -> PathBuf {
        let mut parts = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            parts.push(&self.nodes[index].name);
            index = parent
        }
        self.root.join(parts.into_iter().rev().collect::<PathBuf>())
    }
    fn frame(&mut self, index: usize, fd: File, cancel: &AtomicBool) -> io::Result<Option<Frame>> {
        let mut names = vec![];
        match fs::read_dir(fd_path(&fd)) {
            Err(e) => {
                self.nodes[index].error(e);
                return Ok(None);
            }
            Ok(entries) => {
                for entry in entries {
                    check(cancel)?;
                    match entry {
                        Ok(e) => names.push(e.file_name()),
                        Err(e) => self.nodes[index].error(e),
                    }
                }
            }
        }
        names.sort_unstable();
        Ok(Some(Frame {
            index,
            fd,
            entries: names.into_iter(),
        }))
    }
}
pub fn scan(root: &Path, cancel: &AtomicBool) -> io::Result<Scan> {
    check(cancel)?;
    let fd = open_dir(root)?;
    let meta = fd.metadata()?;
    let mut seen = HashSet::from([(meta.dev(), meta.ino())]);
    let device = meta.dev();
    let mut result = Scan {
        root: root.into(),
        nodes: vec![Node::new(root.as_os_str().into(), None)],
    };
    result.nodes[0].count(&meta);
    let mut stack: Vec<_> = result.frame(0, fd, cancel)?.into_iter().collect();
    while let Some(frame) = stack.last_mut() {
        check(cancel)?;
        let Some(name) = frame.entries.next() else {
            stack.pop();
            continue;
        };
        let parent = frame.index;
        let path = fd_path(&frame.fd).join(&name);
        let mut node = Node::new(name, Some(parent));
        let mut directory = None;
        match fs::symlink_metadata(&path) {
            Err(e) => node.error(e),
            Ok(m) => {
                node.directory = m.is_dir();
                if m.dev() != device {
                    node.note = Some("Other filesystem · skipped".into())
                } else if (m.is_dir() || m.nlink() > 1) && !seen.insert((m.dev(), m.ino())) {
                    node.note = Some("Already counted".into())
                } else {
                    node.count(&m);
                    if m.is_dir() {
                        match open_dir(&path).and_then(|f| {
                            let actual = f.metadata()?;
                            if (actual.dev(), actual.ino()) != (m.dev(), m.ino()) {
                                return Err(io::Error::other("Folder changed during scan"));
                            }
                            Ok(f)
                        }) {
                            Ok(f) => directory = Some(f),
                            Err(e) => node.error(e),
                        }
                    }
                }
            }
        }
        let index = result.nodes.len();
        result.nodes.push(node);
        result.nodes[parent].children.push(index);
        if let Some(fd) = directory
            && let Some(frame) = result.frame(index, fd, cancel)?
        {
            stack.push(frame)
        }
    }
    // Children follow their parents in this flat tree. Sum bottom-up without recursion.
    for index in (1..result.nodes.len()).rev() {
        check(cancel)?;
        let n = &result.nodes[index];
        let (bytes, partial, parent) = (n.bytes, n.partial, n.parent.unwrap());
        result.nodes[parent].bytes.add(bytes);
        result.nodes[parent].partial |= partial;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn links_unreadable_directory_and_cancellation() {
        let dest = tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
            .unwrap();
        let path = dest.path();
        fs::write(path.join("a"), [5; 4096]).unwrap();
        fs::hard_link(path.join("a"), path.join("b")).unwrap();
        symlink("a", path.join("sym")).unwrap();
        let restricted = path.join("private");
        fs::create_dir(&restricted).unwrap();
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000)).unwrap();
        let cancel = AtomicBool::new(false);
        let report = scan(path, &cancel).unwrap();
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(report.nodes[0].bytes.apparent, 4097);
        let hard = report.nodes.iter().find(|n| n.name == "b").unwrap();
        assert_eq!(hard.note.as_deref(), Some("Already counted"));
        assert_eq!(hard.bytes.allocated, 0);
        if !rustix::process::geteuid().is_root() {
            assert!(
                report.nodes[0].partial
                    && report
                        .nodes
                        .iter()
                        .any(|n| n.note.as_deref() == Some("Unreadable · permission denied"))
            )
        }
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            scan(path, &cancel).err().unwrap().kind(),
            io::ErrorKind::Interrupted
        );
    }
}
