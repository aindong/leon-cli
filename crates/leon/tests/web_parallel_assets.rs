//! Review finding (skeptic): a browser fetches the page's assets in parallel, one connection each
//! (`Connection: close`): every one must be served, not closed unanswered by the gate's
//! per-address cap on connections waiting for a decision.

mod support;

use std::time::Duration;
use support::localapi::Serving;
use support::web;

#[test]
fn a_browser_loading_the_page_gets_every_asset() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    s.fake.set_delay(Duration::from_millis(300)); // tailscaled answering whois in 0.3 s
    let port = s.port;
    let assets = ["/theme.css", "/app.css", "/app.js", "/icon.svg", "/conn.js", "/grid.js"];
    let threads: Vec<_> = assets
        .iter()
        .map(|p| {
            let p = p.to_string();
            std::thread::spawn(move || {
                let req = format!("GET {p} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
                let bytes = web::raw(port, req.as_bytes());
                (p, web::parse(&bytes).map(|r| r.status))
            })
        })
        .collect();
    let got: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    let lost: Vec<_> = got.iter().filter(|(_, st)| *st != Some(200)).collect();
    assert!(lost.is_empty(), "assets a browser fetched at once were closed unanswered: {lost:?}");
}
