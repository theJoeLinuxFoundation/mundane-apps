fn main() {
    println!("cargo:rerun-if-changed=src/native.c");
    let packages = ["fontconfig", "pangocairo", "pangofc"];
    let mut build = cc::Build::new();
    for package in packages {
        let library = pkg_config::Config::new()
            .cargo_metadata(false)
            .probe(package)
            .expect("Fontconfig and PangoCairo development files");
        build.includes(library.include_paths);
    }
    build.file("src/native.c").compile("font_preview");
    // With --as-needed, the archive must introduce references before the libraries.
    for package in packages {
        pkg_config::probe_library(package).expect("Fontconfig and PangoCairo libraries");
    }
}
