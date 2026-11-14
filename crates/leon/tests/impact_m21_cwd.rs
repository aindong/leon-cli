//! Optional cwd preserves layout-based clients; explicit cwd has documented precedence.
mod support;
use serde_json::json;
use support::Env;

#[test]
fn existing_layout_cwd_is_preserved_and_top_level_cwd_overrides_it() {
    let env = Env::new();
    let a = env.dir.path().join("a");
    let b = env.dir.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    for (cwd, expected) in [(None, &a), (Some(&b), &b)] {
        let mut params = json!({"layout":{"block":{"options":{"cwd":a}}}});
        if let Some(cwd) = cwd {
            params["cwd"] = json!(cwd);
        }
        let r = env.json(&["api", "call", "session.create", &params.to_string()]);
        let block = r["block_id"].as_str().unwrap();
        env.ok(&["send", "-b", block, "printf 'IMPACT_CWD='; pwd\n"]);
        let want = format!("IMPACT_CWD={}", expected.canonicalize().unwrap().display());
        env.screen_until(block, "preserved cwd semantics", |s| s.contains(&want));
    }
}
