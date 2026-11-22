use super::*;
use std::collections::HashSet;

#[derive(Default, Clone)]
pub(super) struct Selection {
    marked: Option<HashSet<PathBuf>>,
    anchor: Option<usize>,
}
impl Selection {
    pub(super) fn retain(&mut self, entries: &[Entry]) {
        if let Some(marked) = &mut self.marked {
            let remaining: HashSet<_> = entries.iter().map(|e| &e.path).collect();
            marked.retain(|p| remaining.contains(p));
        }
        self.anchor = None;
    }
}
impl Files {
    pub(super) fn selection_count(&self) -> usize {
        self.marks
            .marked
            .as_ref()
            .map_or(usize::from(self.selected.is_some()), HashSet::len)
    }
    pub(super) fn chosen(&self, i: usize) -> bool {
        self.marks
            .marked
            .as_ref()
            .map_or(self.selected == Some(i), |paths| {
                paths.contains(&self.entries[i].path)
            })
    }
    pub(super) fn paths(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(i, _)| self.chosen(*i))
            .map(|(_, e)| e.path.clone())
            .collect()
    }
    pub(super) fn dragged_entries(&self, source: &Path) -> Vec<&Entry> {
        let Some(index) = self.entries.iter().position(|entry| entry.path == source) else {
            return Vec::new();
        };
        if !self.chosen(index) {
            return vec![&self.entries[index]];
        }
        self.entries
            .iter()
            .enumerate()
            .filter(|(i, _)| self.chosen(*i))
            .map(|(_, entry)| entry)
            .collect()
    }
    pub(super) fn import_drop(
        &mut self,
        paths: &ExternalPaths,
        folder: &Path,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sources = paths
            .paths()
            .iter()
            .filter(|path| path.is_absolute())
            .cloned()
            .collect();
        self.transfer_paths(sources, folder, Transfer::Copy, w, cx);
    }
    pub(super) fn pick(&mut self, i: usize, shift: bool, toggle: bool) {
        if shift {
            let anchor = self
                .marks
                .anchor
                .unwrap_or(self.selected.unwrap_or(i))
                .min(self.entries.len() - 1);
            self.marks.anchor = Some(anchor);
            self.marks.marked = Some(
                self.entries[anchor.min(i)..=anchor.max(i)]
                    .iter()
                    .map(|e| e.path.clone())
                    .collect(),
            );
        } else if toggle {
            let cursor = self
                .selected
                .and_then(|j| self.entries.get(j))
                .map(|e| e.path.clone());
            let paths = self
                .marks
                .marked
                .get_or_insert_with(|| cursor.into_iter().collect());
            let path = &self.entries[i].path;
            if !paths.remove(path) {
                paths.insert(path.clone());
            }
            self.marks.anchor = Some(i);
        } else {
            self.marks = Default::default();
        }
        self.selected = Some(i);
        self.message = None;
    }
    pub(super) fn select_all(&mut self) {
        self.marks.marked = Some(self.entries.iter().map(|e| e.path.clone()).collect());
        self.message = None;
    }
    pub(super) fn transfer_paths(
        &mut self,
        paths: Vec<PathBuf>,
        folder: &Path,
        how: Transfer,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.working || self.dialog.is_some() {
            return;
        }
        // A recursive search can select both a directory and one of its children.
        let paths = top_level(paths);
        self.queue = paths
            .into_iter()
            .filter_map(|source| {
                let dest = folder.join(source.file_name()?);
                (how != Transfer::Move || dest != source)
                    .then_some(Job::Paste(Paste { source, dest, how }, PasteAction::New))
            })
            .collect();
        if let Some(job) = self.queue.pop_front() {
            self.execute(job, w, cx);
        }
    }
}
pub(super) fn top_level(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let all: HashSet<_> = paths.iter().cloned().collect();
    paths
        .into_iter()
        .filter(|p| {
            seen.insert(p.clone()) && !p.ancestors().skip(1).any(|parent| all.contains(parent))
        })
        .collect()
}

pub(super) struct Dragged(pub PathBuf);
impl Render for Dragged {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .px_2()
            .py_1()
            .bg(rgb(theme.panel))
            .text_color(rgb(theme.text))
            .child(
                self.0
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            )
    }
}
