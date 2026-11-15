//! M18 criteria 5–7 (.agent/plans/2026-11-15-m18-integrator-promise.md): docs/api.md is rendered
//! from `help.all` and can't drift; docs/api-snapshot.json pins the stable methods' schemas, so a
//! change to one fails until the snapshot is blessed after a version bump; CHANGELOG.md names
//! every milestone that landed and the current api_version.
//!
//! `LEON_BLESS=1 cargo test -p leon-server --test m18_api_docs` rewrites both files.

use std::path::PathBuf;
use std::sync::Arc;

use leon_server::{Ctx, Info};
use serde_json::{json, Map, Value};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bless() -> bool {
    std::env::var_os("LEON_BLESS").is_some_and(|v| v == "1")
}

fn help() -> Value {
    let c = Arc::new(Ctx::new(Info { pid: 1, version: "test", socket: "/tmp/x".into() }, false));
    tokio::runtime::Runtime::new().unwrap().block_on(c.call("help.all", json!(null))).unwrap()
}

const HOW: &str = "LEON_BLESS=1 cargo test -p leon-server --test m18_api_docs";

// ---- docs/api.md -------------------------------------------------------------------------------

/// A schema as a short type: `string`, `[Node]`, `"a" | "b"`, `{ block: BlockSpec }`.
fn ty(s: &Value) -> String {
    if let Some(r) = s["$ref"].as_str() {
        return format!("`{}`", r.rsplit('/').next().unwrap());
    }
    if let Some(c) = s.get("const") {
        return format!("`{c}`");
    }
    if let Some(e) = s["enum"].as_array() {
        return e.iter().map(|v| format!("`{v}`")).collect::<Vec<_>>().join(" | ");
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(alts) = s[key].as_array() {
            return alts.iter().map(ty).collect::<Vec<_>>().join(" | ");
        }
    }
    let one = |t: &str| -> String {
        match t {
            "array" => format!("[{}]", ty(&s["items"])),
            "object" => match s["properties"].as_object() {
                Some(p) if !p.is_empty() => {
                    let req = required(s);
                    let f: Vec<String> = p
                        .iter()
                        .map(|(k, v)| format!("{k}{}: {}", if req.contains(k) { "" } else { "?" }, ty(v)))
                        .collect();
                    format!("{{ {} }}", f.join(", "))
                }
                _ => "object".into(),
            },
            "integer" => match s["format"].as_str() {
                Some(f) => format!("integer ({f})"),
                None => "integer".into(),
            },
            t => t.into(),
        }
    };
    match &s["type"] {
        Value::String(t) => one(t),
        Value::Array(ts) => ts.iter().filter_map(Value::as_str).map(one).collect::<Vec<_>>().join(" | "),
        _ => "any".into(),
    }
}

fn required(s: &Value) -> Vec<String> {
    s["required"].as_array().map_or(vec![], |r| r.iter().filter_map(|v| v.as_str().map(String::from)).collect())
}

fn cell(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace('|', "\\|")
}

/// The fields of an object schema as a table, or a line saying there are none.
fn table(out: &mut String, s: &Value) {
    let props = s["properties"].as_object().filter(|p| !p.is_empty());
    let Some(props) = props else {
        let what = if s["type"] == "object" { "none".to_string() } else { ty(s) };
        out.push_str(&format!("{what}\n\n"));
        return;
    };
    let req = required(s);
    out.push_str("\n\n| Field | Type | Required | Description |\n|---|---|---|---|\n");
    for (k, v) in props {
        let desc = v["description"].as_str().unwrap_or("");
        let yes = if req.contains(k) { "yes" } else { "" };
        out.push_str(&format!("| `{k}` | {} | {yes} | {} |\n", cell(&ty(v)), cell(desc)));
    }
    out.push('\n');
}

fn method(out: &mut String, m: &Value) {
    let name = m["name"].as_str().unwrap();
    out.push_str(&format!(
        "### `{name}`\n\n*{}*. {}\n\n",
        m["stability"].as_str().unwrap(),
        m["description"].as_str().unwrap()
    ));
    if let Some(d) = m.get("deprecated") {
        out.push_str(&format!(
            "**Deprecated** since {}: use `{}` instead; removed in {}.\n\n",
            d["since"].as_str().unwrap_or("?"),
            d["replacement"].as_str().unwrap_or("?"),
            d["removal"].as_str().unwrap_or("?")
        ));
    }
    for (label, key) in [("Input", "input_schema"), ("Output", "output_schema")] {
        out.push_str(&format!("**{label}:** "));
        table(out, &m[key]);
    }
    let mut defs: Vec<(String, Value)> = vec![];
    for key in ["input_schema", "output_schema"] {
        for (k, v) in m[key]["$defs"].as_object().into_iter().flatten() {
            if !defs.iter().any(|(d, _)| d == k) {
                defs.push((k.clone(), v.clone()));
            }
        }
    }
    for (k, v) in &defs {
        let desc = v["description"].as_str().map(|d| format!(" {}", cell(d))).unwrap_or_default();
        if v["properties"].as_object().is_some_and(|p| !p.is_empty()) {
            out.push_str(&format!("**`{k}`:**{desc}"));
            table(out, v);
        } else {
            out.push_str(&format!("**`{k}`:** {}.{desc}\n\n", ty(v)));
        }
    }
    if let (Some(req), Some(resp)) = (m.get("request_example"), m.get("response_example")) {
        out.push_str("**Example:**\n\n```json\n");
        out.push_str(&format!("{}\n", json!({"id": 1, "method": name, "params": req})));
        out.push_str(&format!("{}\n```\n\n", json!({"id": 1, "result": resp})));
    }
    let schemas = json!({"input_schema": m["input_schema"], "output_schema": m["output_schema"]});
    out.push_str("<details><summary>JSON Schemas</summary>\n\n```json\n");
    out.push_str(&serde_json::to_string_pretty(&schemas).unwrap());
    out.push_str("\n```\n\n</details>\n\n");
}

fn anchor(heading: &str) -> String {
    heading
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .collect::<String>()
        .replace(' ', "-")
}

fn render(h: &Value) -> String {
    let mut groups: Vec<(String, Vec<&Value>)> = vec![];
    for m in h["methods"].as_array().unwrap() {
        let ns = m["name"].as_str().unwrap().split('.').next().unwrap().to_string();
        match groups.iter_mut().find(|(g, _)| *g == ns) {
            Some((_, ms)) => ms.push(m),
            None => groups.push((ns, vec![m])),
        }
    }
    let mut out = String::new();
    out.push_str(
        "# leon API reference\n\n\
         <!-- Generated from `help.all` by crates/leon-server/tests/m18_api_docs.rs; don't edit by hand.\n     \
         Change the registry, then run `LEON_BLESS=1 cargo test -p leon-server --test m18_api_docs`. -->\n\n",
    );
    out.push_str(&format!(
        "API version **{}**. Every method is marked:\n\n\
         - *stable*: covered by the [compatibility policy](integrators.md#compatibility). It only gains\n  \
         optional fields until the next major version.\n\
         - *experimental*: may change or go away in any release.\n\
         - *internal*: for leon's own clients; don't build on it.\n\n\
         Call a method by writing one JSON line to the server's socket, such as\n\
         `{{\"id\":1,\"method\":\"session.list\",\"params\":{{}}}}`; [integrators.md](integrators.md)\n\
         explains the socket, the framing, errors and events. `leon api describe METHOD` prints the\n\
         same facts for the server you run.\n\n",
        h["api_version"].as_str().unwrap()
    ));
    out.push_str("## Contents\n\n");
    for (g, ms) in &groups {
        out.push_str(&format!("- [{g}](#{}): {}\n", anchor(g), ms.len()));
    }
    for t in h["block_types"].as_array().unwrap() {
        let heading = format!("Block methods: {}", t["creator"].as_str().unwrap());
        out.push_str(&format!("- [{heading}](#{}): {}\n", anchor(&heading), t["methods"].as_array().unwrap().len()));
    }
    out.push('\n');
    for (g, ms) in &groups {
        out.push_str(&format!("## {g}\n\n"));
        ms.iter().for_each(|m| method(&mut out, m));
    }
    for t in h["block_types"].as_array().unwrap() {
        let creator = t["creator"].as_str().unwrap();
        out.push_str(&format!("## Block methods: {creator}\n\n"));
        let flavors: Vec<String> =
            t["flavors"].as_array().unwrap().iter().map(|f| format!("`{}`", f.as_str().unwrap())).collect();
        out.push_str(&format!(
            "A block made by `{creator}` (flavors: {}) answers these through `block.method`: pass\n\
             the method's full name as `method` and its input as `args`. The output is `block.method`'s result.\n\n",
            flavors.join(", ")
        ));
        t["methods"].as_array().unwrap().iter().for_each(|m| method(&mut out, m));
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

#[test]
fn api_md_matches_the_registry() {
    let want = render(&help());
    let path = repo().join("docs/api.md");
    if bless() {
        std::fs::write(&path, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    if have != want {
        let line = have.lines().zip(want.lines()).position(|(a, b)| a != b).map_or(0, |i| i + 1);
        panic!("docs/api.md drifted from help.all (first difference near line {line}); run `{HOW}`");
    }
}

// ---- docs/api-snapshot.json --------------------------------------------------------------------

/// A schema without its prose and with its refs inlined, so a doc edit or a renamed Rust type
/// isn't an API change. A ref's sibling keywords (`default`) are kept; a recursive ref becomes
/// `{"$ref": "#<depth>"}`, the level of the type it points back to.
fn normal(s: &Value, defs: &Map<String, Value>, stack: &mut Vec<String>) -> Value {
    match s {
        Value::Object(o) => {
            let mut out = Map::new();
            if let Some(r) = o.get("$ref").and_then(Value::as_str) {
                let name = r.rsplit('/').next().unwrap().to_string();
                if let Some(depth) = stack.iter().position(|n| *n == name) {
                    out.insert("$ref".into(), json!(format!("#{depth}")));
                } else {
                    stack.push(name.clone());
                    if let Value::Object(inlined) = normal(&defs[&name], defs, stack) {
                        out = inlined;
                    }
                    stack.pop();
                }
            }
            for (k, v) in o {
                let v = match k.as_str() {
                    "$ref" | "description" | "title" | "$schema" | "$defs" => continue,
                    // field names, not keywords: every one is kept
                    "properties" => match v.as_object() {
                        Some(p) => Value::Object(p.iter().map(|(f, x)| (f.clone(), normal(x, defs, stack))).collect()),
                        None => normal(v, defs, stack),
                    },
                    _ => normal(v, defs, stack),
                };
                out.insert(k.clone(), v);
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(|v| normal(v, defs, stack)).collect()),
        v => v.clone(),
    }
}

fn snapshot(h: &Value) -> Value {
    let mut methods = Map::new();
    let blocks = h["block_types"].as_array().unwrap().iter().flat_map(|t| t["methods"].as_array().unwrap());
    for m in h["methods"].as_array().unwrap().iter().chain(blocks) {
        if m["stability"] != "stable" {
            continue;
        }
        let mut one = Map::new();
        for key in ["input_schema", "output_schema"] {
            let defs = m[key]["$defs"].as_object().cloned().unwrap_or_default();
            one.insert(key.into(), normal(&m[key], &defs, &mut vec![]));
        }
        methods.insert(m["name"].as_str().unwrap().into(), Value::Object(one));
    }
    json!({ "api_version": h["api_version"], "methods": methods })
}

/// What differs between the committed snapshot's methods and the current ones.
fn changes(old: &Value, new: &Value) -> Vec<String> {
    let mut changed = vec![];
    for (k, v) in new.as_object().unwrap() {
        match old.get(k) {
            None => changed.push(format!("added: {k}")),
            Some(o) if o != v => changed.push(format!("changed: {k}")),
            _ => {}
        }
    }
    for k in old.as_object().unwrap().keys().filter(|k| new.get(k.as_str()).is_none()) {
        changed.push(format!("removed (or no longer stable): {k}"));
    }
    changed
}

fn semver(v: &Value) -> (u64, u64, u64) {
    let n: Vec<u64> = v.as_str().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect();
    (n.first().copied().unwrap_or(0), n.get(1).copied().unwrap_or(0), n.get(2).copied().unwrap_or(0))
}

/// Whether blessing may write `new` over `old`. A change to the stable methods needs a higher
/// api_version: a removal a major one, any other change at least a minor one (a patch changes no
/// schema). It can't tell an added optional field from a breaking edit; that choice is the author's.
fn may_bless(old: Option<&Value>, new: &Value) -> Result<(), String> {
    let Some(old) = old else { return Ok(()) };
    let changed = changes(&old["methods"], &new["methods"]);
    let (was, now) = (semver(&old["api_version"]), semver(&new["api_version"]));
    if changed.is_empty() {
        return if now >= was { Ok(()) } else { Err(format!("API_VERSION went down from {was:?} to {now:?}")) };
    }
    let removed = changed.iter().any(|c| c.starts_with("removed"));
    let need = if removed { "a major" } else { "at least a minor" };
    let ok = if removed { now.0 > was.0 } else { now > was && (now.0, now.1) != (was.0, was.1) };
    if ok {
        return Ok(());
    }
    Err(format!(
        "the stable API changed ({}) but API_VERSION {} isn't {need} bump over {}; \
         see docs/integrators.md#compatibility",
        changed.join(", "),
        new["api_version"],
        old["api_version"]
    ))
}

#[test]
fn bless_needs_a_new_version_for_a_changed_api() {
    let snap = |v: &str, m: Value| json!({"api_version": v, "methods": m});
    let a = json!({"x.y": {"input_schema": {"type": "object"}}});
    let b = json!({"x.y": {"input_schema": {"type": "object", "properties": {"z": {}}}}});
    assert!(may_bless(None, &snap("1.0.0", a.clone())).is_ok(), "the first snapshot");
    assert!(may_bless(Some(&snap("1.0.0", a.clone())), &snap("1.0.0", a.clone())).is_ok(), "nothing changed");
    assert!(may_bless(Some(&snap("1.0.0", a.clone())), &snap("1.0.0", b.clone())).is_err(), "changed, same version");
    assert!(may_bless(Some(&snap("1.0.0", a.clone())), &snap("1.0.0", json!({}))).is_err(), "removed, same version");
    assert!(may_bless(Some(&snap("1.0.0", a.clone())), &snap("1.1.0", b.clone())).is_ok(), "changed with a minor bump");
    assert!(
        may_bless(Some(&snap("1.0.0", a.clone())), &snap("1.0.1", b.clone())).is_err(),
        "a patch changes no schema"
    );
    assert!(may_bless(Some(&snap("1.1.0", a.clone())), &snap("1.0.0", b.clone())).is_err(), "a lower version");
    assert!(may_bless(Some(&snap("1.0.0", a.clone())), &snap("1.1.0", json!({}))).is_err(), "a removal needs a major");
    assert!(may_bless(Some(&snap("1.0.0", a.clone())), &snap("2.0.0", json!({}))).is_ok(), "removed with a major bump");
    assert!(
        may_bless(Some(&snap("1.1.0", a.clone())), &snap("1.0.0", a)).is_err(),
        "no change, but the version went down"
    );
}

#[test]
fn stable_schemas_match_the_snapshot() {
    let want = snapshot(&help());
    assert!(want["methods"].get("dev.leon.terminal.write").is_some(), "stable block methods are in the snapshot");
    let path = repo().join("docs/api-snapshot.json");
    let have: Option<Value> = std::fs::read_to_string(&path).ok().map(|t| serde_json::from_str(&t).unwrap());
    if have.as_ref() == Some(&want) {
        return;
    }
    if bless() {
        may_bless(have.as_ref(), &want).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(&path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
        return;
    }
    let empty = json!({});
    let changed = changes(have.as_ref().map_or(&empty, |h| &h["methods"]), &want["methods"]);
    panic!(
        "the stable API no longer matches docs/api-snapshot.json:\n  {}\nsnapshot api_version {}, \
         API_VERSION {}.\nIf this is deliberate, bump API_VERSION by docs/integrators.md#compatibility, \
         note it in CHANGELOG.md, and run `{HOW}`.",
        if changed.is_empty() { "api_version only".to_string() } else { changed.join("\n  ") },
        have.as_ref().map_or(Value::Null, |h| h["api_version"].clone()),
        want["api_version"]
    );
}

#[test]
fn a_deprecated_method_shows_its_replacement() {
    let mut out = String::new();
    let m = json!({
        "name": "x.old", "stability": "stable", "description": "Old.",
        "deprecated": {"since": "1.2.0", "replacement": "x.new", "removal": "2.0.0"},
        "input_schema": {"type": "object"}, "output_schema": {"type": "object"},
    });
    method(&mut out, &m);
    assert!(out.contains("**Deprecated** since 1.2.0: use `x.new` instead; removed in 2.0.0."), "{out}");
}

// ---- CHANGELOG.md -------------------------------------------------------------------------------

#[test]
fn changelog_names_every_landed_milestone_and_the_api_version() {
    let log = std::fs::read_to_string(repo().join("CHANGELOG.md")).expect("CHANGELOG.md exists")
        // the milestones' sections moved here (file-size limit), headings and all
        + &std::fs::read_to_string(repo().join("docs/changelog-milestones.md")).expect("the milestones");
    assert!(
        log.contains("Keep a Changelog") && log.contains("Semantic Versioning"),
        "the header names its conventions"
    );
    assert!(log.contains("## [Unreleased]"), "an Unreleased section");
    let headings: Vec<&str> = log.lines().filter(|l| l.starts_with("## ")).collect();
    for tag in ["M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8", "M9", "M10", "M11", "M14", "M16a", "M16b", "M17"] {
        let found = headings.iter().any(|h| {
            h.match_indices(tag).any(|(at, _)| !h[at + tag.len()..].starts_with(|c: char| c.is_ascii_alphanumeric()))
        });
        assert!(found, "no `## ` heading for {tag}");
    }
    assert!(log.contains("haven't landed"), "says which milestones haven't landed yet");
    let v = help()["api_version"].as_str().unwrap().to_string();
    assert!(log.contains("api_version"), "says how to read api_version");
    assert!(log.contains(&format!("`{v}`")), "names the current api_version `{v}`");
}
