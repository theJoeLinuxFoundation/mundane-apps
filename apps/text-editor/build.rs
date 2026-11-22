fn main() {
    println!("cargo:rerun-if-changed=src/print.c");
    let library = pkg_config::Config::new()
        .cargo_metadata(false)
        .probe("pangocairo")
        .expect("PangoCairo development files");
    cc::Build::new()
        .includes(library.include_paths)
        .file("src/print.c")
        .compile("text_print");
    pkg_config::probe_library("pangocairo").expect("PangoCairo libraries");
}
