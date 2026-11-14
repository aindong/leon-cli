mod support;
use support::Env;
#[test]
fn visible_overlay_can_focus_back_to_tiles() {
    let e = Env::new();
    let (sid, tile) = e.session();
    let float = e.ok(&["-s", &sid, "float", "--new"]);
    e.ok(&["-s", &sid, "focus", "prev"]);
    assert_eq!(e.window(&sid, 0)["focused_block_id"], tile, "prev leaves float for tile");
    e.ok(&["focus", &float]);
    e.ok(&["-s", &sid, "focus", "left"]);
    assert_eq!(e.window(&sid, 0)["focused_block_id"], tile, "direction leaves overlay for tiles");
}
#[test]
fn picker_lists_floating_blocks() {
    use support::host::{attach_then_stty, Host};
    let e = Env::new();
    let (sid, _) = e.session();
    e.ok(&["session", "rename", &sid, "m20picker"]);
    e.ok(&["-s", &sid, "float", "--new", "--", "sleep", "301"]);
    let h = Host::spawn(&e, &attach_then_stty(&sid), &[]);
    h.wait_text("m20picker");
    h.send(b"\x02s");
    h.wait_text("Pick a session, tab or block");
    h.send(b"301");
    h.wait_text("sleep 301");
}
