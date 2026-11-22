use mundane_services::packages::Dave;
use std::path::PathBuf;
fn main() {
    let dir = PathBuf::from(std::env::args_os().nth(1).expect("fixture directory"));
    mundane_packages::run(Dave::with_programs(dir.join("dave"), dir.join("pkexec")));
}
