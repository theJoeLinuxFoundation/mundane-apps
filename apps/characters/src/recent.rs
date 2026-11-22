use std::{fs, io, path::PathBuf};
fn path() -> io::Result<PathBuf> {
    let root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or(io::ErrorKind::NotFound)?;
    Ok(root.join("mundane-characters/recent.json"))
}
pub fn load() -> io::Result<Vec<String>> {
    match fs::read(path()?) {
        Ok(b) => Ok(serde_json::from_slice(&b)?),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e),
    }
}
pub fn remember(s: &str) -> io::Result<()> {
    let path = path()?;
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir)?;
    let lock = fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("recent.lock"))?;
    lock.lock()?;
    let mut list = load()?;
    list.retain(|v| v != s);
    list.insert(0, s.to_owned());
    list.truncate(32);
    let file = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer(&file, &list)?;
    file.persist(path)?;
    Ok(())
}
