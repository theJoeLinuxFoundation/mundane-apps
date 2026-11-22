use mundane_services::playback::TrackInfo;
use std::path::PathBuf;

pub struct Track {
    pub id: u64,
    pub path: PathBuf,
    pub info: TrackInfo,
    pub error: Option<String>,
}
impl Track {
    pub fn title(&self) -> String {
        self.info.display_title(&self.path)
    }
}
#[derive(Default)]
pub struct Queue {
    pub tracks: Vec<Track>,
    pub current: Option<u64>,
    pub selected: Option<u64>,
    last_id: u64,
}
impl Queue {
    pub fn append(&mut self, path: PathBuf) -> u64 {
        self.last_id += 1;
        let id = self.last_id;
        self.tracks.push(Track {
            id,
            path,
            info: Default::default(),
            error: None,
        });
        self.selected.get_or_insert(id);
        id
    }
    pub fn position(&self, id: Option<u64>) -> Option<usize> {
        let id = id?;
        self.tracks.iter().position(|t| t.id == id)
    }
    pub fn neighbor(&self, id: Option<u64>, by: isize) -> Option<u64> {
        let next = self.position(id)?.checked_add_signed(by)?;
        self.tracks.get(next).map(|t| t.id)
    }
    pub fn move_selected(&mut self, by: isize) {
        if let Some(i) = self.position(self.selected)
            && let Some(j) = i.checked_add_signed(by)
            && j < self.tracks.len()
        {
            self.tracks.swap(i, j)
        }
    }
    pub fn remove_selected(&mut self) -> Option<u64> {
        let i = self.position(self.selected)?;
        let removed = self.tracks.remove(i);
        self.selected = self
            .tracks
            .get(i)
            .or_else(|| self.tracks.last())
            .map(|t| t.id);
        Some(removed.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eos_uses_queue_order_and_missing_tags_use_filename() {
        let mut q = Queue::default();
        let a = q.append("a.wav".into());
        let b = q.append("folder/untagged.wav".into());
        let c = q.append("a.wav".into());
        q.current = Some(a);
        q.selected = Some(c);
        q.move_selected(-1);
        assert_eq!(q.neighbor(q.current, 1), Some(c));
        assert_eq!(q.neighbor(Some(c), 1), Some(b));
        assert_eq!(q.neighbor(Some(b), 1), None);
        assert_eq!(q.tracks[2].title(), "untagged.wav");
        assert_eq!(q.remove_selected(), Some(c));
        assert_eq!(q.selected, Some(b));
    }
}
