//! The snapshot must keep fields that happen to be named `title` or `description`; only the
//! schema keywords of that name are prose.

use std::path::PathBuf;

use serde_json::Value;

#[test]
fn the_snapshot_keeps_fields_named_title_and_description() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/api-snapshot.json");
    let snap: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let m = &snap["methods"];
    let block = &m["session.describe_block"]["output_schema"]["properties"];
    assert!(block.get("title").is_some(), "session.describe_block's `title` field escapes the snapshot: {block}");
    let item = &m["session.list_blocks"]["output_schema"]["properties"]["blocks"]["items"]["properties"];
    assert!(item.get("title").is_some(), "session.list_blocks' `title` field escapes the snapshot: {item}");
    let doc = &m["help.all"]["output_schema"]["properties"]["methods"]["items"]["properties"];
    assert!(doc.get("description").is_some(), "help.all's `description` field escapes the snapshot: {doc}");
}
