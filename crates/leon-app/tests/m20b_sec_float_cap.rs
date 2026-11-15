//! Review (security): a remote server's float list is capped, so a hostile one can't make every
//! frame quadratic; one block placed twice is a view a server should never send.

use leon_app::remote;
use leon_core::overlays::Overlays;
use serde_json::json;

fn float(id: &str) -> serde_json::Value {
    json!({ "block_id": id, "bounds": { "x": 0.1, "y": 0.1, "width": 0.5, "height": 0.5 }, "hidden": false })
}

#[test]
fn a_hostile_number_of_floats_is_not_kept() {
    let many: Vec<_> = (0..100_000).map(|i| float(&format!("b{i}"))).collect();
    let kept = remote::decode_overlays(&json!({ "window_id": "w", "floating": many })).floating.len();
    assert!(kept <= 1024, "kept {kept} floats from one server reply");
}

#[test]
fn one_block_placed_twice_gives_no_overlays() {
    let twice = json!({ "window_id": "w", "floating": [float("a"), float("a")] });
    assert_eq!(remote::decode_overlays(&twice), Overlays::default(), "one block floated twice");
    let both = json!({ "window_id": "w", "floating": [float("a")],
        "docked": [{ "block_id": "a", "side": "left", "size": 0.3, "hidden": false }] });
    assert_eq!(remote::decode_overlays(&both), Overlays::default(), "one block floated and docked");
}

#[test]
fn overlay_ids_are_read_as_block_ids_are() {
    let w = json!({ "window_id": "w", "floating": [float("a\u{200b}b")] });
    assert_eq!(remote::decode_overlays(&w).floating[0].block_id, "ab", "hidden characters stripped");
}
