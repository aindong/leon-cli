//! What each command does.

use std::time::{Duration, Instant};

use leon_core::paths::socket_path;
use leon_core::ApiError;
use serde_json::{json, Value};

use crate::client::{connect, spawn_server, Client, Fail};
use crate::{ApiCmd, Cli, Cmd, ConfigCmd, ServerCmd, SessionCmd, WindowCmd};

pub fn run(cli: &Cli) -> Result<(), Fail> {
    let socket = socket_path();
    if let Cmd::App = cli.cmd() {
        // the app is a client: the server comes first, as `leon server start` makes it
        start_server(&socket, true, cli.autostart).inspect_err(|e| leon_app::alert(&message(e)))?;
        return leon_app::run(&socket).map_err(Fail::Other);
    }
    if let Cmd::Server(cmd) = cli.cmd() {
        return server(cli, cmd);
    }
    if let Cmd::Config(ConfigCmd::Check { path }) = cli.cmd() {
        return config_check(path.clone());
    }
    if let Cmd::Skill(cmd) = cli.cmd() {
        return crate::skill::run(cli, cmd);
    }
    let mut c = connect(&socket, cli.autostart)?;
    match cli.cmd() {
        Cmd::App | Cmd::Server(_) | Cmd::Config(ConfigCmd::Check { .. }) | Cmd::Skill(_) => unreachable!(),
        Cmd::Config(ConfigCmd::Reload) => crate::script::reload(&mut c, cli)?,
        Cmd::Do(args) => crate::script::run(&mut c, cli, args, &socket)?,
        Cmd::Actions => crate::script::actions(&mut c, cli)?,
        Cmd::Keymap => crate::script::keymap(&mut c, cli)?,
        Cmd::New { label } | Cmd::Session(SessionCmd::New { label }) => {
            let r = c.call("session.create", json!({ "label": label }))?;
            println!("{}", r["session_id"].as_str().unwrap_or_default());
        }
        Cmd::Ls | Cmd::Session(SessionCmd::Ls) => {
            let r = c.call("session.list", json!({}))?;
            let sessions = r["sessions"].as_array().cloned().unwrap_or_default();
            if cli.json {
                print_json(&Value::Array(sessions));
            } else {
                for (i, s) in sessions.iter().enumerate() {
                    let n = s["windows"].as_u64().unwrap_or(0);
                    println!(
                        "{}  {}  {n} window{}  {}",
                        i + 1,
                        label(&s["label"]),
                        if n == 1 { "" } else { "s" },
                        s["session_id"].as_str().unwrap_or("")
                    );
                }
            }
        }
        Cmd::Kill { sessions: targets } | Cmd::Session(SessionCmd::Kill { targets }) => {
            let mut ids = targets.iter().map(|t| resolve(&mut c, "session", t, None)).collect::<Result<Vec<_>, _>>()?;
            ids.sort(); // `kill web 1` may name one session twice
            ids.dedup();
            for id in ids {
                c.call("session.destroy", json!({ "session_id": id }))?;
            }
        }
        Cmd::Session(SessionCmd::Inspect { target }) => {
            let sid = pick_session(&mut c, cli, target.as_deref())?;
            let v = c.call("session.view", json!({ "session_id": sid }))?;
            if cli.json {
                print_json(&v);
            } else {
                println!("{}  {}", label(&v["label"]), sid);
                print_windows(v["windows"].as_array().map(Vec::as_slice).unwrap_or_default());
            }
        }
        Cmd::Session(SessionCmd::Rename { target, label }) => {
            let sid = resolve(&mut c, "session", target, None)?;
            let r = c.call("session.set_label", json!({ "session_id": sid, "label": label }))?;
            json_out(cli, &r);
        }
        Cmd::Window(cmd) => window(&mut c, cli, cmd)?,
        Cmd::Run { .. }
        | Cmd::Send { .. }
        | Cmd::SendKey { .. }
        | Cmd::Capture { .. }
        | Cmd::Wait { .. }
        | Cmd::Block(_) => crate::blocks::run(&mut c, cli)?,
        Cmd::Split { .. }
        | Cmd::Focus { .. }
        | Cmd::Move { .. }
        | Cmd::Swap { .. }
        | Cmd::Zoom { .. }
        | Cmd::Resize { .. } => crate::layout::run(&mut c, cli)?,
        Cmd::Events { all } => crate::events::run(&mut c, cli, *all)?,
        Cmd::Api(cmd) => api(&mut c, cli, cmd)?,
    }
    Ok(())
}

/// Exit 1 when the load fails or a `leon.*` call was skipped; warnings and logs don't fail it.
fn config_check(path: Option<std::path::PathBuf>) -> Result<(), Fail> {
    let path = path
        .or_else(leon_lua::config_path)
        .ok_or_else(|| Fail::Other("no config path: set LEON_CONFIG or HOME".into()))?;
    let cfg = leon_lua::load(&path).map_err(|e| Fail::Other(e.to_string()))?;
    let n = |n: usize, what: &str| format!("{n} {what}{}", if n == 1 { "" } else { "s" });
    println!(
        "{}: {}, {}, {}",
        path.display(),
        n(cfg.bindings.len(), "binding"),
        n(cfg.modes.len(), "mode"),
        n(cfg.actions.len(), "action")
    );
    for d in &cfg.diagnostics {
        println!("{d}");
    }
    for l in &cfg.logs {
        println!("{}: log.{}: {}", l.at, l.level.as_str(), l.message);
    }
    match cfg.diagnostics.iter().filter(|d| d.level == leon_lua::Level::Error).count() {
        0 => Ok(()),
        errors => Err(Fail::Other(format!("{} in {}", n(errors, "error"), path.display()))),
    }
}

fn window(c: &mut Client, cli: &Cli, cmd: &WindowCmd) -> Result<(), Fail> {
    let sid = pick_session(c, cli, None)?;
    match cmd {
        WindowCmd::New { label } => {
            let r = c.call("session.new_window", json!({ "session_id": sid, "window_label": label }))?;
            match cli.json {
                true => print_json(&r),
                false => println!("{}", r["window_id"].as_str().unwrap_or_default()),
            }
        }
        WindowCmd::Ls => {
            let v = c.call("session.view", json!({ "session_id": sid }))?;
            let windows = v["windows"].as_array().cloned().unwrap_or_default();
            if cli.json {
                print_json(&Value::Array(windows));
            } else {
                print_windows(&windows);
            }
        }
        WindowCmd::Rename { target, label } => {
            let wid = resolve(c, "window", target, Some(&sid))?;
            let r =
                c.call("session.set_window_label", json!({ "session_id": sid, "window_id": wid, "label": label }))?;
            json_out(cli, &r);
        }
        WindowCmd::Close { target } => {
            let wid = resolve(c, "window", target, Some(&sid))?;
            let r = c.call("session.close_window", json!({ "session_id": sid, "window_id": wid }))?;
            json_out(cli, &r);
        }
    }
    Ok(())
}

fn api(c: &mut Client, cli: &Cli, cmd: &ApiCmd) -> Result<(), Fail> {
    let help = c.call("help.all", json!({}))?;
    let methods = help["methods"].as_array().cloned().unwrap_or_default();
    match cmd {
        ApiCmd::List => {
            if cli.json {
                let brief: Vec<Value> =
                    methods.iter().map(|m| json!({ "name": m["name"], "description": m["description"] })).collect();
                print_json(&Value::Array(brief));
            } else {
                for m in &methods {
                    println!("{:<28} {}", m["name"].as_str().unwrap_or(""), m["description"].as_str().unwrap_or(""));
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
                println!("{}\n  {}\n", method, m["description"].as_str().unwrap_or(""));
                println!("input:\n{}\n", pretty(&m["input_schema"]));
                println!("output:\n{}", pretty(&m["output_schema"]));
            }
        }
        ApiCmd::Call { method, args } => {
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

/// A running server (perhaps autostarted on demand) is told to stay up; otherwise one starts,
/// detached and persistent, unless `may_start` is false.
fn start_server(socket: &std::path::Path, restore: bool, may_start: bool) -> Result<(), Fail> {
    // an idle autostarted server can accept and then exit: then start a fresh one
    let persisted = Client::connect(socket).map(|mut c| c.call("server.persist", json!({})));
    match persisted {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(Fail::Unreachable(_))) | Err(_) if may_start => spawn_server(socket, false, restore),
        Ok(Err(f)) => Err(f),
        Err(_) => Err(Fail::Unreachable(format!("no leon server at {}", socket.display()))),
    }
}

/// What a failure says, for the alert a Finder launch shows instead of stderr.
fn message(f: &Fail) -> String {
    match f {
        Fail::Api(e) => e.to_string(),
        Fail::Unreachable(m) | Fail::Other(m) => m.clone(),
        Fail::Exit(code) => format!("exit status {code}"),
    }
}

fn server(cli: &Cli, cmd: &ServerCmd) -> Result<(), Fail> {
    let socket = socket_path();
    match cmd {
        ServerCmd::Run { autostarted, restore, config } => {
            let idle = std::env::var("LEON_IDLE_EXIT_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10);
            let info = leon_server::Info {
                pid: std::process::id(),
                version: env!("CARGO_PKG_VERSION"),
                socket: socket.display().to_string(),
            };
            let ctx = std::sync::Arc::new(leon_server::Ctx::new(info, *autostarted));
            let persist = leon_core::paths::state_dir()
                .map(|d| leon_server::persist::Persist { file: d.join(leon_server::persist::FILE), restore: *restore });
            let opts = leon_server::serve::Options { socket, idle_exit: Duration::from_secs(idle) };
            let rt = tokio::runtime::Runtime::new().map_err(|e| Fail::Other(e.to_string()))?;
            let serve = async {
                ctx.start_config(config.clone());
                let mut bound = leon_server::serve::listen(&opts.socket).await?;
                if let Some(p) = &persist {
                    bound.persist(&ctx, p); // after binding: a server that loses the race keeps nothing
                }
                leon_server::serve::serve_on(ctx, bound, opts).await
            };
            rt.block_on(serve).map_err(|e| Fail::Other(e.to_string()))
        }
        ServerCmd::Start { restore } => {
            if !restore && Client::connect(&socket).is_ok() {
                eprintln!("leon: a server is already running; --restore=false applies only when one starts");
            }
            start_server(&socket, *restore, true)
        }
        ServerCmd::Stop { discard } => {
            let mut c = connect(&socket, false)?;
            // Plain stops send no params, which servers from before `discard` also take.
            let params = if *discard { json!({ "discard": true }) } else { json!({}) };
            c.call("server.stop", params)?;
            let end = Instant::now() + Duration::from_secs(5);
            while Instant::now() < end && Client::connect(&socket).is_ok() {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(())
        }
        ServerCmd::Status => {
            let mut c = connect(&socket, false)?;
            let st = c.call("server.status", json!({}))?;
            if cli.json {
                print_json(&st);
            } else {
                println!(
                    "running  pid {}  version {}  {} session(s)  {}",
                    st["pid"],
                    st["version"].as_str().unwrap_or(""),
                    st["sessions"],
                    st["socket"].as_str().unwrap_or("")
                );
            }
            Ok(())
        }
    }
}

pub(crate) fn resolve(c: &mut Client, kind: &str, query: &str, session_id: Option<&str>) -> Result<String, Fail> {
    let r = c.call("target.resolve", json!({ "kind": kind, "query": query, "session_id": session_id }))?;
    Ok(r["id"].as_str().unwrap_or_default().to_string())
}

/// The session a command acts on: an explicit target, then `-s`, then `$LEON_SESSION`, then the
/// only session if there is exactly one.
pub(crate) fn pick_session(c: &mut Client, cli: &Cli, explicit: Option<&str>) -> Result<String, Fail> {
    find_session(c, cli, explicit)?
        .ok_or_else(|| Fail::Api(ApiError::not_found("which session? pass -s or set LEON_SESSION")))
}

/// `pick_session`, but None when nothing names a session and there isn't exactly one.
pub(crate) fn find_session(c: &mut Client, cli: &Cli, explicit: Option<&str>) -> Result<Option<String>, Fail> {
    let env = std::env::var("LEON_SESSION").ok().filter(|s| !s.is_empty());
    if let Some(q) = explicit.or(cli.session.as_deref()).or(env.as_deref()) {
        return resolve(c, "session", q, None).map(Some);
    }
    let r = c.call("session.list", json!({}))?;
    Ok(match r["sessions"].as_array().map(Vec::as_slice) {
        Some([only]) => Some(only["session_id"].as_str().unwrap_or_default().to_string()),
        _ => None,
    })
}

pub(crate) fn read_args(args: Option<&str>) -> Result<Value, Fail> {
    let text = match args {
        None => return Ok(json!({})),
        Some("-") => std::io::read_to_string(std::io::stdin()).map_err(|e| Fail::Other(e.to_string()))?,
        Some(a) if a.starts_with('@') => {
            std::fs::read_to_string(&a[1..]).map_err(|e| Fail::Other(format!("{}: {e}", &a[1..])))?
        }
        Some(a) => a.to_string(),
    };
    serde_json::from_str(&text).map_err(|e| Fail::Other(format!("params are not JSON: {e}")))
}

fn print_windows(windows: &[Value]) {
    for w in windows {
        let mark = if w["active"] == true { "*" } else { " " };
        println!("{}{mark} {}  {}", w["position"], label(&w["label"]), w["window_id"].as_str().unwrap_or(""));
    }
}

pub(crate) fn label(v: &Value) -> &str {
    v.as_str().unwrap_or("-")
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

pub(crate) fn print_json(v: &Value) {
    println!("{}", pretty(v));
}

/// Print a mutation's reply (its `revision`) when `--json` asks for it.
pub(crate) fn json_out(cli: &Cli, v: &Value) {
    if cli.json {
        print_json(v);
    }
}
