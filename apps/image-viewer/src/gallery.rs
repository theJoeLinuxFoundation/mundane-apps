use super::Picture;
use std::path::{Path, PathBuf};
#[derive(Default, Clone)]
pub(super) struct Gallery {
    paths: Vec<PathBuf>,
    index: usize,
}
impl Gallery {
    pub fn for_path(path: &Path) -> Self {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
        let mut paths: Vec<_> = path
            .parent()
            .and_then(|p| p.read_dir().ok())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| !e.file_name().as_encoded_bytes().starts_with(b"."))
            .map(|e| e.path())
            .filter(|p| {
                p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    ["png", "jpg", "jpeg", "webp", "svg"]
                        .iter()
                        .any(|s| e.eq_ignore_ascii_case(s))
                }) && p.is_file()
            })
            .collect();
        paths.push(path.clone());
        paths.sort();
        paths.dedup();
        let index = paths.binary_search(&path).unwrap();
        Self { paths, index }
    }
    pub fn neighbour(&self, delta: isize) -> Option<usize> {
        self.index
            .checked_add_signed(delta)
            .filter(|&i| i < self.paths.len())
    }
    pub fn step(mut self, delta: isize) -> anyhow::Result<(Picture, Self)> {
        let mut error = None;
        while let Some(i) = self.neighbour(delta) {
            self.index = i;
            match Picture::open(self.paths[i].clone()) {
                Ok(picture) => return Ok((picture, self)),
                Err(e) => error = Some(e),
            }
        }
        Err(error.unwrap_or_else(|| anyhow::anyhow!("No more images in this direction.")))
    }
}
