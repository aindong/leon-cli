//! Serve over Tailscale: a let-in connection whose first bytes aren't an API request (an HTTP
//! request, say) is closed unread, and none of its lines runs as a request.

mod support;

use support::localapi::Serving;

#[test]
fn a_connection_that_is_not_the_api_is_closed_unread() {
    let s = Serving::start("");
    let body = "{\"id\":1,\"method\":\"server.stop\"}\n";
    let http = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
        s.port,
        body.len()
    );
    for first in [http.as_str(), "\n{\"id\":1,\"method\":\"server.stop\"}\n"] {
        let got = s.exchange(first);
        assert!(got.is_empty(), "closed unread, with no reply: {:?}", String::from_utf8_lossy(&got));
        assert!(s.env.socket().exists(), "the server.stop in it never ran");
    }
    assert_eq!(s.status_over_tcp()["result"]["state"], "running", "API callers still get in");
}
