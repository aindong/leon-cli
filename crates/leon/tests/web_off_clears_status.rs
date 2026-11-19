//! Review finding (correctness, cross-model): turning serve_web off while serving clears the web
//! URLs in server.status.
mod support;
use support::localapi::Serving;

#[test]
fn turning_serve_web_off_clears_the_web_urls() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    s.until("the web URL", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
    let out = s.cmd().args(["config", "set", "serve_web", "false"]).output().unwrap();
    assert!(out.status.success(), "config set: {}", String::from_utf8_lossy(&out.stderr));
    s.until("the web URL gone", |t| t["web"] == serde_json::json!([]));
}
