//! What the API promises integrators (docs/integrators.md#compatibility): its version, each
//! method's stability, deprecations, and request/response examples. Every method, block methods
//! included, has a line in `STABILITY`; a test fails when one is missing or names no method.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Value};

use crate::methods::MethodDoc;

/// The API's semver, apart from the build's version. A change to a stable method needs a bump:
/// docs/api-snapshot.json won't take the change without one.
pub const API_VERSION: &str = "1.0.0";

#[derive(Serialize, JsonSchema, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Stability {
    /// Covered by the compatibility policy: until the next major version it only gains optional fields.
    Stable,
    /// May change or go away in any release.
    Experimental,
    /// For leon's own clients (the app, `leon attach`); don't build on it.
    Internal,
}

#[derive(Serialize, JsonSchema, Clone, Debug)]
pub struct Deprecated {
    /// The api_version that deprecated the method.
    pub since: String,
    /// The method to call instead.
    pub replacement: String,
    /// The api_version that removes it (a major one).
    pub removal: String,
}

use Stability::{Experimental, Internal, Stable};

const STABILITY: &[(&str, Stability)] = &[
    ("help.all", Stable),
    ("server.status", Stable),
    ("server.persist", Stable),
    ("server.version", Stable),
    ("server.stop", Stable),
    ("server.config_reload", Stable),
    ("session.list", Stable),
    ("session.create", Stable),
    ("session.destroy", Stable),
    ("session.set_label", Stable),
    ("session.view", Stable),
    ("session.new_window", Stable),
    ("session.close_window", Stable),
    ("session.focus_window", Stable),
    ("session.set_window_label", Stable),
    ("target.resolve", Stable),
    ("session.list_blocks", Stable),
    ("session.describe_block", Stable),
    ("block.close", Stable),
    ("block.method", Stable),
    ("block.event", Stable),
    ("block.wait", Stable),
    ("session.new_split", Stable),
    ("session.move_block", Stable),
    ("session.swap_blocks", Stable),
    ("session.zoom_block", Stable),
    ("session.resize_pane", Stable),
    ("session.focus_block", Stable),
    ("session.focus_direction", Stable),
    ("session.focus_next_pane", Stable),
    ("session.focus_previous_pane", Stable),
    ("session.attach", Stable),
    ("session.attach_all", Stable),
    ("session.resume", Stable),
    // The screen transport is still moving with the app (M10, M11).
    ("block.watch", Experimental),
    ("client.action", Experimental),
    ("client.key", Experimental),
    ("client.keymap", Internal),
    ("dev.leon.terminal.write", Stable),
    ("dev.leon.terminal.format", Stable),
    ("dev.leon.terminal.size", Stable),
    ("dev.leon.terminal.resize", Stable),
    ("dev.leon.terminal.pwd", Stable),
    ("dev.leon.terminal.screen", Experimental),
    ("dev.leon.terminal.process", Stable),
    ("dev.leon.terminal.set_resume", Stable),
    ("dev.leon.terminal.program_status", Stable),
    ("dev.leon.terminal.lines", Experimental),
    ("dev.leon.terminal.find", Experimental),
    ("dev.leon.terminal.selection", Experimental),
    ("dev.leon.terminal.link_at", Experimental),
];

/// (method, since, replacement, removal). None yet.
const DEPRECATED: &[(&str, &str, &str, &str)] = &[];

/// Every method the table has a line for.
pub fn named() -> impl Iterator<Item = &'static str> {
    STABILITY.iter().map(|(n, _)| *n)
}

/// The doc `help.all` shows for a method.
pub(crate) fn doc(name: &str, description: &str, input_schema: Value, output_schema: Value) -> MethodDoc {
    // A method missing from the table is promised nothing; the contract test fails on it anyway.
    let stability = STABILITY.iter().find(|(n, _)| *n == name).map_or(Experimental, |(_, s)| *s);
    let deprecated = DEPRECATED.iter().find(|d| d.0 == name).map(|d| Deprecated {
        since: d.1.into(),
        replacement: d.2.into(),
        removal: d.3.into(),
    });
    let (request_example, response_example) = example(name).unzip();
    MethodDoc {
        name: name.into(),
        description: description.into(),
        stability,
        deprecated,
        input_schema,
        output_schema,
        request_example,
        response_example,
    }
}

const SID: &str = "session:5b1c2f0e-8d7a-4c39-9a61-2e4f7b0c9d13";
const WID: &str = "window:0f3e9a42-7c15-4b8e-a2d6-91c4e5b7f028";
const BID: &str = "block:c7d2e816-3a9f-4f05-8b1e-6d0a4c2f9b57";

/// (params, result) for the methods integrators call most.
fn example(name: &str) -> Option<(Value, Value)> {
    Some(match name {
        "server.version" => (json!({}), json!({ "version": "0.1.0", "api_version": API_VERSION })),
        "session.list" => (json!({}), json!({ "sessions": [{ "session_id": SID, "label": "web", "windows": 2 }] })),
        "session.create" => (
            json!({ "label": "web", "layout": { "block": { "options": {
                "command": ["ls", "-l"], "cwd": "/tmp", "exit": { "on_completion": false } } } } }),
            json!({ "session_id": SID, "window_id": WID, "block_id": BID }),
        ),
        "target.resolve" => (json!({ "kind": "session", "query": "web" }), json!({ "id": SID, "session_id": null })),
        "session.new_split" => (
            json!({ "session_id": SID, "anchor_id": BID, "direction": "horizontal", "side": "after",
                    "layout": { "block": { "label": "logs" } } }),
            json!({ "block_id": "block:9e41b0d3-52c8-4a7f-b6e2-0d1f8c3a5e74", "window_id": WID, "revision": 4 }),
        ),
        "block.method" => (
            json!({ "session_id": SID, "block_id": BID, "method": "dev.leon.terminal.format", "args": { "trim": true } }),
            json!({ "text": "$ ls\nREADME.md  src\n$" }),
        ),
        "block.event" => {
            (json!({ "session_id": SID, "block_id": BID, "event": { "type": "key", "key": "ctrl+c" } }), json!({}))
        }
        "block.wait" => (json!({ "block_id": BID, "timeout_ms": 60000 }), json!({ "exit_code": 0 })),
        "session.attach_all" => (
            json!({ "event_scope": "session", "resumable": true }),
            json!({ "seq": 0, "sessions": [{ "session_id": SID, "label": "web", "windows": 1 }],
                    "resume": "resume:3d8f1a6c-0b2e-4c97-a5d4-7e9b2f1c0a68" }),
        ),
        "session.resume" => {
            (json!({ "resume": "resume:3d8f1a6c-0b2e-4c97-a5d4-7e9b2f1c0a68", "seq": 41 }), json!({ "seq": 41 }))
        }
        "dev.leon.terminal.write" => (json!({ "data": "ls\r" }), json!({})),
        "dev.leon.terminal.format" => {
            (json!({ "format": "text", "trim": true }), json!({ "text": "$ ls\nREADME.md  src\n$" }))
        }
        _ => return None,
    })
}
