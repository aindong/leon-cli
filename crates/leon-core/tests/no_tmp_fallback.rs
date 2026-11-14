#[test]
fn runtime_dir_never_falls_back_to_shared_tmp() {
    std::env::remove_var("LEON_RUNTIME_DIR");
    std::env::remove_var("XDG_RUNTIME_DIR");
    std::env::remove_var("HOME");
    let d = leon_core::paths::runtime_dir();
    assert!(!d.starts_with("/tmp/.local"), "SHARED_TMP_FALLBACK: {}", d.display());
}
