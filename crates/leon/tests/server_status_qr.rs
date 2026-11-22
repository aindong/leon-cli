//! `leon server status --qr` draws the web page's QR code under the status, from the same encoder
//! as the Settings window (whose decode tests are crates/leon-app/tests/qr_decodes.rs); with no
//! web page it says why and the status still succeeds.

mod support;
use support::localapi::Serving;

#[test]
fn qr_draws_the_web_pages_url() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    let t = s.until("the web URL", |t| t["web"][0].is_string());
    let url = t["web"][0].as_str().unwrap().to_string();
    let out = s.cmd().args(["server", "status", "--qr"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(&format!("web      {url}")), "{text}");
    let want = leon_app::qr::terminal_lines(&url, false).unwrap().join("\n");
    assert!(text.contains(&want), "piped: the bare half blocks of {url}:\n{text}");
    assert!(!text.contains('\x1b'), "no colours into a pipe");
}

#[test]
fn qr_without_a_web_page_says_why() {
    let s = Serving::start("");
    let out = s.cmd().args(["server", "status", "--qr"]).output().unwrap();
    assert!(out.status.success(), "the status itself worked");
    let (text, err) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(!text.contains('█'), "no code: {text}");
    assert!(err.contains("serve_web"), "says why: {err}");
}
