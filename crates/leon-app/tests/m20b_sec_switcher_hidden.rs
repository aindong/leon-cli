//! Review (security): a float named by its program shows nothing hidden or reordered from a
//! remote's argv, and its label is stripped as every remote name is.

use std::collections::HashMap;

use leon_app::remote::{Block, Model, Session};
use leon_app::switcher;
use leon_core::overlays::{Bounds, Floating, Overlays};
use leon_core::Window;

#[test]
fn a_float_row_named_by_its_program_hides_nothing() {
    let o = Overlays {
        floating: vec![Floating { block_id: "b".into(), bounds: Bounds::default(), hidden: false }],
        docked: vec![],
    };
    let window = Window { id: "w".into(), label: Some("tab".into()), layout: None, focus: None, zoomed: None };
    let block = Block {
        id: "b".into(),
        window_id: "w".into(),
        label: None,
        command: vec!["/usr/bin/\u{202e}hs.lave".into()],
        title: None,
        cwd: None,
        foreground: None,
        status: vec![],
    };
    let s = Session {
        id: "s".into(),
        label: None,
        revision: 1,
        active: 0,
        windows: vec![window],
        blocks: vec![block],
        overlays: HashMap::from([("w".to_string(), o)]),
        tiled_focus: HashMap::new(),
    };
    let m = Model { sessions: std::sync::Arc::new(vec![s]), ..Default::default() };
    let (_, blocks) = switcher::tabs(&[(1, "devbox", &m)]);
    let text = blocks[0].text();
    assert!(!text.contains('\u{202e}'), "a bidi override reached the switcher: {text:?}");
}
