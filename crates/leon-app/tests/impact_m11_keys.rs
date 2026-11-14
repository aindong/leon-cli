//! M11 impact: the scroll-paging keys are the app's alone; `leon attach` (which shares the keymap)
//! still passes shift+PageUp to the pane, where programs like less and weechat use it.

use leon_app::keymap::{Keymap, Pressed};
use leon_lua::keys::parse;
use serde_json::json;

fn press(k: &Keymap, key: &str) -> Pressed {
    k.press(&[], &parse(key).unwrap().seq[0])
}

#[test]
fn page_keys_bind_in_the_app_only() {
    for key in ["shift+pageup", "shift+pagedown"] {
        assert_eq!(press(&Keymap::new(&json!({})), key), Pressed::Unbound, "{key} reaches the pane in leon attach");
        assert!(matches!(press(&Keymap::app(&json!({})), key), Pressed::Run(_)), "{key} pages the app's view");
    }
    assert!(matches!(press(&Keymap::new(&json!({})), "cmd+c"), Pressed::Run(_)), "cmd keys never reach a terminal");
}
