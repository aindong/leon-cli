//! Review proof (cross-model): a rename field the sidebar hides as it closes (Toggle Sidebar)
//! ends, so the focus never stays on a field that isn't drawn: typed text reaches the pane.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn a_rename_field_hidden_with_the_sidebar_gives_the_keys_back(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    let row = center(at(cx, "row-0-0").expect("the tab row"));
    double_click(cx, row);
    assert_eq!(owner(&ws, cx), Some("rename"), "the field has the focus");
    act(&ws, cx, "client.sidebar.toggle");
    assert!(!ws.read_with(cx, |ws, _| ws.sidebar_visible()), "the sidebar is hidden");
    let r = back_on_pane(&s, &ws, cx, "hidden1");
    s.app.stop();
    r.unwrap();
}
