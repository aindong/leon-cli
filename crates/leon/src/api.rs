//! `leon api list/describe/call`: the server's methods from `help.all`, block methods included.

use leon_core::ApiError;
use serde_json::{json, Value};

use crate::client::{Client, Fail};
use crate::commands::{pretty, print_json, read_args, resolve};
use crate::{ApiCmd, Cli};

pub(crate) fn run(c: &mut Client, cli: &Cli, cmd: &ApiCmd) -> Result<(), Fail> {
    let help = c.call("help.all", json!({}))?;
    let mut methods = help["methods"].as_array().cloned().unwrap_or_default();
    let registry = methods.len(); // after these come the block methods
    for t in help["block_types"].as_array().into_iter().flatten() {
        methods.extend(t["methods"].as_array().into_iter().flatten().cloned());
    }
    let is_block = |name: &str| methods[registry..].iter().any(|m| m["name"] == name);
    let through = |name: &str| format!("{name} is a block method: call it with `leon block call {name} [ARGS]`");
    let text = |m: &Value, k: &str| m[k].as_str().unwrap_or("").to_string();
    match cmd {
        ApiCmd::List => {
            if cli.json {
                let brief: Vec<Value> = methods
                    .iter()
                    .map(|m| {
                        let mut b =
                            json!({ "name": m["name"], "stability": m["stability"], "description": m["description"] });
                        if let Some(d) = m.get("deprecated") {
                            b["deprecated"] = d.clone();
                        }
                        b
                    })
                    .collect();
                print_json(&Value::Array(brief));
            } else {
                for m in &methods {
                    println!("{:<34} {:<13} {}", text(m, "name"), text(m, "stability"), text(m, "description"));
                }
            }
        }
        ApiCmd::Describe { method } => {
            let m = methods
                .iter()
                .find(|m| m["name"] == method.as_str())
                .ok_or_else(|| Fail::Other(ApiError::method_not_found(method).to_string()))?;
            if cli.json {
                print_json(m);
            } else {
                print!("{}", describe(m));
                if is_block(method) {
                    println!("\n{} (or `block.method`)", through(method));
                }
            }
        }
        ApiCmd::Call { method, args } => {
            if is_block(method) {
                return Err(Fail::Other(through(method)));
            }
            let mut params = read_args(args.as_deref())?;
            if let Some(q) = &cli.session {
                let sid = resolve(c, "session", q, None)?;
                if let Some(obj) = params.as_object_mut() {
                    obj.insert("session_id".into(), json!(sid));
                }
            }
            // A failed call is a failed command (exit 1), even when the error is a missing target.
            let r = c.call(method, params).map_err(|f| match f {
                Fail::Api(e) => Fail::Other(e.to_string()),
                f => f,
            })?;
            print_json(&r);
        }
    }
    Ok(())
}

/// A method doc as `leon api describe` prints it.
fn describe(m: &Value) -> String {
    let s = |k: &str| m[k].as_str().unwrap_or("?").to_string();
    let mut out = format!("{} ({})\n  {}\n\n", s("name"), s("stability"), s("description"));
    if let Some(d) = m.get("deprecated") {
        let f = |k: &str| d[k].as_str().unwrap_or("?").to_string();
        out += &format!(
            "deprecated since {}: use {} instead; removed in {}\n\n",
            f("since"),
            f("replacement"),
            f("removal")
        );
    }
    out += &format!("input:\n{}\n\noutput:\n{}\n", pretty(&m["input_schema"]), pretty(&m["output_schema"]));
    for (label, key) in [("request example", "request_example"), ("response example", "response_example")] {
        if let Some(e) = m.get(key) {
            out += &format!("\n{label}:\n{}\n", pretty(e));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn describe_names_the_replacement_of_a_deprecated_method() {
        let m = json!({
            "name": "x.old", "stability": "stable", "description": "Old.",
            "deprecated": {"since": "1.2.0", "replacement": "x.new", "removal": "2.0.0"},
            "input_schema": {}, "output_schema": {},
        });
        let out = super::describe(&m);
        assert!(out.starts_with("x.old (stable)\n"), "{out}");
        assert!(out.contains("deprecated since 1.2.0: use x.new instead; removed in 2.0.0"), "{out}");
    }
}
