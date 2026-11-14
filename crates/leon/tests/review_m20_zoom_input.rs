mod support;
use support::host::{attach_then_stty, Host};
use support::Env;
#[test]
fn focused_float_receives_keys_above_zoomed_tile() {
    let e = Env::new();
    let (sid, tile) = e.session();
    let float = e.ok(&["-s", &sid, "float", "--new", "--", "/bin/sh"]);
    e.ok(&["zoom", "-b", &tile, "--on"]);
    let h = Host::spawn(&e, &attach_then_stty(&sid), &[]);
    h.wait_text("┌");
    h.send(b"printf 'FLOAT_%s\\n' INPUT\r");
    h.wait_text("FLOAT_INPUT");
    assert!(e.capture(&float).contains("FLOAT_INPUT"));
}
