fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--watch-alarms")) {
        if let Some(path) = mundane_services::clocks::settings_path()
            && let Err(e) =
                mundane_services::clocks::alarms::watch(&path.with_file_name("alarms.json"))
        {
            eprintln!("Could not watch alarms: {e}");
            std::process::exit(1);
        }
    } else {
        mundane_clocks::run(None)
    }
}
