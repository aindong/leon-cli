//! M22a review (correctness): an init.lua that is a symlink to a file not made yet (a dotfiles repo
//! not set up) stays a symlink; the new file is written at its target.

#[test]
fn dangling_symlink_config_stays_a_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("init.lua");
    std::os::unix::fs::symlink(dir.path().join("real.lua"), &link).unwrap();
    leon_lua::rewrite::write_setting(&link, "font.size", &serde_json::json!(14)).unwrap();
    let ft = std::fs::symlink_metadata(&link).unwrap().file_type();
    assert!(ft.is_symlink(), "dangling link replaced by a regular file");
    assert!(std::fs::read_to_string(dir.path().join("real.lua")).unwrap().contains("14"));
}
