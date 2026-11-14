mod support;
use support::Env;
#[test]
fn previous_visits_each_float() {
    let e = Env::new();
    let (sid, _) = e.session();
    let a = e.ok(&["-s", &sid, "float", "--new", "--", "sleep", "301"]);
    e.ok(&["-s", &sid, "float", "--new", "--", "sleep", "302"]);
    e.ok(&["-s", &sid, "float", "--new", "--", "sleep", "303"]);
    e.ok(&["-s", &sid, "focus", "prev"]);
    e.ok(&["-s", &sid, "focus", "prev"]);
    assert_eq!(e.window(&sid, 0)["focused_block_id"], a, "previous should reach the first float");
}
