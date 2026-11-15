//! M22a review (correctness): a `leon.set` written in a comment on the call's line is not the call;
//! `config set` must not rewrite the comment and report success.

#[test]
fn comment_is_not_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("init.lua");
    let src = "local size = 14\nleon.set(\"font.size\", size) -- was leon.set(\"font.size\", 13)\n";
    std::fs::write(&p, src).unwrap();
    let _ = leon_lua::rewrite::write_setting(&p, "font.size", &serde_json::json!(14));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), src, "comment rewritten");
}
