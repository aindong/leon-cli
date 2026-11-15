#[test]
fn a_remote_panes_directory_is_the_hosts_to_check() {
    let only_there = std::path::PathBuf::from("/dd-leon-only-on-the-host");
    assert_eq!(super::split_cwd(Some(only_there.clone()), true).as_deref(), Some("/dd-leon-only-on-the-host"));
    assert_eq!(super::split_cwd(Some(only_there), false), None, "here, a gone dir falls back to home");
    assert_eq!(super::split_cwd(None, true), None);
}
