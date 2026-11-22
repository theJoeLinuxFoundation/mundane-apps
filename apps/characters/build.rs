fn main() {
    println!("cargo:rerun-if-changed=src/names.c");
    let icu = pkg_config::probe_library("icu-uc").expect("ICU common development files");
    cc::Build::new()
        .file("src/names.c")
        .includes(icu.include_paths)
        .compile("character_names");
}
