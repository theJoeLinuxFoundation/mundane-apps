#[test]
fn suite_binary_dependency_licences() {
    let status = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/dependency_licences.py"
        ))
        .status()
        .expect("run workspace dependency licence check");
    assert!(status.success());
}
