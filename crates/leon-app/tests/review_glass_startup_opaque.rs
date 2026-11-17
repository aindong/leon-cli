mod m9_support;

use gpui::{TestAppContext, WindowBackgroundAppearance as Bg};
use m9_support::*;

#[gpui::test]
fn translucency_0_never_asks_for_a_blurred_window(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"translucency\", 0)\n");
    let (ws, cx) = open(cx, &s.app);
    let set = ws.read_with(cx, |ws, _| ws.backgrounds_set().to_vec());
    s.app.stop();
    assert!(!set.contains(&Bg::Blurred), "a window set to 0 was blurred first: {set:?}");
}
