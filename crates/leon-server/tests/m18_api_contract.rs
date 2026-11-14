//! M18 criteria 1–3 (.agent/plans/2026-11-15-m18-integrator-promise.md): `help.all` and
//! `server.version` carry `api_version`; every method and block method has a stability marker;
//! the most-used methods carry request and response examples that match their schemas.

use std::sync::Arc;

use leon_server::{Ctx, Info};
use serde_json::{json, Value};

fn ctx() -> Arc<Ctx> {
    Arc::new(Ctx::new(Info { pid: 1, version: "test", socket: "/tmp/x".into() }, false))
}

async fn help(c: &Arc<Ctx>) -> Value {
    c.call("help.all", json!(null)).await.unwrap()
}

/// Every method doc: the registry's and the block methods.
fn docs(help: &Value) -> Vec<Value> {
    let mut all = help["methods"].as_array().unwrap().clone();
    for t in help["block_types"].as_array().unwrap() {
        all.extend(t["methods"].as_array().unwrap().iter().cloned());
    }
    all
}

fn find<'a>(docs: &'a [Value], name: &str) -> &'a Value {
    docs.iter().find(|m| m["name"] == name).unwrap_or_else(|| panic!("no method {name}"))
}

#[tokio::test]
async fn api_version_is_semver_and_reported_twice() {
    let c = ctx();
    let h = help(&c).await;
    let v = h["api_version"].as_str().expect("help.all has api_version");
    let parts: Vec<&str> = v.split('.').collect();
    assert_eq!(parts.len(), 3, "{v} is MAJOR.MINOR.PATCH");
    assert!(parts.iter().all(|p| !p.is_empty() && p.parse::<u64>().is_ok()), "{v} is MAJOR.MINOR.PATCH");
    let version = c.call("server.version", json!({})).await.unwrap();
    assert_eq!(version["api_version"], v);
    assert_eq!(version["version"], "test", "the build version stays");
}

#[tokio::test]
async fn every_method_has_a_stability() {
    let h = help(&ctx()).await;
    let all = docs(&h);
    for m in &all {
        let s = m["stability"].as_str().unwrap_or_else(|| panic!("{} has no stability", m["name"]));
        assert!(["stable", "experimental", "internal"].contains(&s), "{}: {s}", m["name"]);
    }
    for name in ["session.create", "block.wait", "session.attach_all", "dev.leon.terminal.write"] {
        assert_eq!(find(&all, name)["stability"], "stable", "{name}");
    }
    assert_eq!(find(&all, "block.watch")["stability"], "experimental");
    assert_eq!(find(&all, "client.keymap")["stability"], "internal");
}

#[test]
fn the_contract_table_and_the_methods_match() {
    let h = tokio::runtime::Runtime::new().unwrap().block_on(async { help(&ctx()).await });
    let names: Vec<String> = docs(&h).iter().map(|m| m["name"].as_str().unwrap().to_string()).collect();
    let table: Vec<&str> = leon_server::contract::named().collect();
    for name in &table {
        assert!(names.iter().any(|n| n == name), "the contract table names {name}, which is no method");
    }
    for name in &names {
        assert!(table.contains(&name.as_str()), "{name} has no entry in the contract table");
    }
}

#[tokio::test]
async fn deprecated_has_a_replacement_and_a_removal_version() {
    let h = help(&ctx()).await;
    let help_all = docs(&h).into_iter().find(|m| m["name"] == "help.all").unwrap();
    let d = &help_all["output_schema"]["$defs"]["Deprecated"]["properties"];
    for field in ["since", "replacement", "removal"] {
        assert!(d.get(field).is_some(), "a method doc's `deprecated` has {field}: {d}");
    }
    let doc = &help_all["output_schema"]["$defs"]["MethodDoc"]["properties"];
    for field in ["stability", "deprecated", "request_example", "response_example"] {
        assert!(doc.get(field).is_some(), "a method doc declares {field}");
    }
}

/// The top-level fields an output schema declares, and the ones it requires.
fn fields(schema: &Value) -> (Vec<String>, Vec<String>) {
    let props = schema["properties"].as_object().map_or(vec![], |p| p.keys().cloned().collect());
    let required =
        schema["required"].as_array().map_or(vec![], |r| r.iter().map(|v| v.as_str().unwrap().to_string()).collect());
    (props, required)
}

#[tokio::test]
async fn most_used_methods_have_examples_that_fit() {
    let h = help(&ctx()).await;
    let all = docs(&h);
    let with: Vec<&Value> = all.iter().filter(|m| m.get("request_example").is_some()).collect();
    assert!(with.len() >= 8, "only {} methods have examples", with.len());
    for name in ["session.list", "session.create", "block.wait", "block.method"] {
        let m = find(&all, name);
        assert!(m.get("request_example").is_some() && m.get("response_example").is_some(), "{name} has examples");
    }
    assert!(with.iter().any(|m| m["name"].as_str().unwrap().starts_with("dev.leon.")), "a block method has examples");
    assert!(find(&all, "server.stop").get("request_example").is_none(), "no example stops the server");
    for m in with {
        let name = m["name"].as_str().unwrap();
        let resp = m.get("response_example").unwrap_or_else(|| panic!("{name}: a request example without a response"));
        let (props, required) = fields(&m["output_schema"]);
        let obj = resp.as_object().unwrap_or_else(|| panic!("{name}: the response example is an object"));
        for k in obj.keys() {
            assert!(props.contains(k), "{name}: the response example has {k}, which the output schema lacks");
        }
        for k in &required {
            assert!(obj.contains_key(k), "{name}: the response example lacks required {k}");
        }
        // The params parse on a fresh server: the call may fail on made-up IDs, never as a bad
        // request. block.method's `args` is only read once the block exists, so those examples
        // run against a live terminal.
        let c = ctx();
        let req = m["request_example"].clone();
        let (method, params) = if name == "block.method" || name.starts_with("dev.leon.") {
            let made = c.call("session.create", json!({})).await.unwrap();
            let (sid, bid) = (made["session_id"].clone(), made["block_id"].clone());
            if name == "block.method" {
                let mut p = req.clone();
                p["session_id"] = sid;
                p["block_id"] = bid;
                ("block.method", p)
            } else {
                ("block.method", json!({"session_id": sid, "block_id": bid, "method": name, "args": req}))
            }
        } else {
            (name, req.clone())
        };
        if let Err(e) = c.call(method, params).await {
            assert_ne!(e.code, "bad_request", "{name}: {req} is rejected: {}", e.message);
        }
    }
}
