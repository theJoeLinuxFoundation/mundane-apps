use mundane_services::processes::Procfs;
use std::sync::Arc;

fn main() {
    mundane_system_monitor::run(Arc::new(Procfs));
}
