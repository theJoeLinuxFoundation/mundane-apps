//! Local HTTP fixture shared by the provider acceptance tests.
use std::{
    path::Path,
    process::{Child, Command},
    time::{Duration, Instant},
};
pub struct Server {
    pub dir: tempfile::TempDir,
    pub address: String,
    child: Child,
}
impl Server {
    pub fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = tempfile::tempdir_in(root.join("target")).unwrap();
        let child = Command::new("python3")
            .arg(root.join("tests/providers/server.py"))
            .arg(dir.path())
            .spawn()
            .unwrap();
        let mut this = Self {
            address: String::new(),
            dir,
            child,
        };
        let start = Instant::now();
        loop {
            if let Ok(s) = std::fs::read_to_string(this.dir.path().join("address"))
                && s.starts_with("http://127.0.0.1:")
            {
                this.address = s;
                break;
            }
            assert!(
                this.child.try_wait().unwrap().is_none()
                    && start.elapsed() < Duration::from_secs(5)
            );
            std::thread::sleep(Duration::from_millis(10))
        }
        this
    }
    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop()
    }
}
