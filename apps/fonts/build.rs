fn main() {
    println!("cargo:rerun-if-changed=src/native.c");
    let mut build = cc::Build::new();
    for package in ["fontconfig", "pangocairo", "pangofc"] {
        let library = pkg_config::probe_library(package)
            .expect("Fontconfig and PangoCairo development files");
        build.includes(library.include_paths);
    }
    build.file("src/native.c").compile("font_preview");
}
