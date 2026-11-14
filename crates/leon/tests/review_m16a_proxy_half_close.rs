//! M16a correctness review: `leon server proxy` answers a client that sends its requests and then
//! closes its side, as the server's socket does, instead of exiting before the replies come.

mod support;

use std::io::Write;
use std::process::Stdio;

use support::Env;

#[test]
fn the_proxy_answers_before_it_exits() {
    let env = Env::new();
    let mut child = env.cmd().args(["server", "proxy"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"{\"id\":7,\"method\":\"server.version\"}\n").unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\"id\":7") && text.contains("result"), "no reply: {text:?}");
    assert!(out.status.success());
}
