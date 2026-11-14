//! A `$ref`'s sibling keywords (`default`) are part of the stable schema: the snapshot keeps them.

use serde_json::Value;

fn find<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                if k == key {
                    out.push(x);
                }
                find(x, key, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| find(x, key, out)),
        _ => {}
    }
}

#[test]
fn ref_siblings_are_pinned() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/api-snapshot.json");
    let snap: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut mods = vec![];
    find(&snap["methods"]["block.event"]["input_schema"], "mods", &mut mods);
    assert!(!mods.is_empty(), "block.event has mods");
    assert!(mods.iter().all(|m| m.get("default").is_some()), "mods default is not pinned: {mods:?}");
}
