//! What each command does.

use leon_core::paths::socket_path;
use leon_core::ApiError;
use serde_json::{json, Value};

use crate::client::{Client, Fail};
use crate::{Cli, Cmd, ConfigCmd, ServerCmd, SessionCmd, WindowCmd};

pub fn run(cli: &Cli) -> Result<(), Fail> {
    // a headless `leon` alone is `leon attach`, which takes -H
    let attaches = cfg!(not(all(feature = "app", target_os = "macos"))) && cli.cmd.is_none();
    if let (Some(what), Some(_), false) = (local_only(cli.cmd()), &cli.host, attaches) {
        return Err(Fail::Usage(format!("`leon {what}` is about this machine; it doesn't take -H")));
    }
    let socket = socket_path();
    if let Cmd::App = cli.cmd() {
        return crate::server::app(cli, &socket);
    }
    if let Cmd::Server(cmd) = cli.cmd() {
        return crate::server::server(cli, cmd);
    }
    if let Cmd::Config(ConfigCmd::Check { path }) = cli.cmd() {
        return config_check(path.clone());
    }
    if let Cmd::Config(cmd @ (ConfigCmd::Get { .. } | ConfigCmd::Set { .. })) = cli.cmd() {
        return crate::settings::config(cli, cmd);
    }
    if let Cmd::Theme(cmd) = cli.cmd() {
        return crate::settings::theme(cli, cmd);
    }
    if let Cmd::Skill(cmd) = cli.cmd() {
        return crate::skill::run(cli, cmd);
    }
    if let Cmd::Hosts(cmd) = cli.cmd() {
        return crate::hosts::run(cli, cmd);
    }
    if let Cmd::SelfUpdate { check } = cli.cmd() {
        return crate::update::run(*check);
    }
    // Validate before connecting/autostarting: a bad local path is a usage error.
    let new_cwd = match cli.cmd() {
        Cmd::New { cwd, .. } | Cmd::Session(SessionCmd::New { cwd, .. }) if cli.host.is_some() => {
            remote_cwd(cwd.as_deref())?
        }
        Cmd::New { cwd, .. } | Cmd::Session(SessionCmd::New { cwd, .. }) => {
            cwd.as_deref().map(new_directory).transpose()?
        }
        Cmd::Run { cwd, .. } | Cmd::Split { cwd, .. } if cli.host.is_some() => {
            remote_cwd(cwd.as_deref())?;
            None
        }
        _ => None,
    };
    let server = crate::hosts::server(cli, cli.autostart)?;
    if let Cmd::Attach { target, prefix } = cli.cmd() {
        let target = target.as_deref().or(cli.session.as_deref());
        return crate::attach::run(target, prefix.as_deref(), cli.autostart, &server);
    }
    let mut c = server.connect(cli.autostart)?;
    match cli.cmd() {
        Cmd::App
        | Cmd::Attach { .. }
        | Cmd::Server(_)
        | Cmd::Config(ConfigCmd::Check { .. } | ConfigCmd::Get { .. } | ConfigCmd::Set { .. })
        | Cmd::Theme(_)
        | Cmd::Skill(_)
        | Cmd::Hosts(_)
        | Cmd::SelfUpdate { .. } => unreachable!(),
        Cmd::Config(ConfigCmd::Reload) => crate::script::reload(&mut c, cli)?,
        Cmd::Do(args) => crate::script::run(&mut c, cli, args, &server.socket)?,
        Cmd::Actions => crate::script::actions(&mut c, cli)?,
        Cmd::Keymap => crate::script::keymap(&mut c, cli)?,
        Cmd::New { label, .. } | Cmd::Session(SessionCmd::New { label, .. }) => {
            let mut params = json!({ "label": label });
            if let Some(cwd) = new_cwd {
                params["cwd"] = json!(cwd);
            }
            let r = c.call("session.create", params)?;
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
        | Cmd::Notify(_)
        | Cmd::Block(_) => crate::blocks::run(&mut c, cli)?,
        Cmd::Float { .. }
        | Cmd::Toggle { .. }
        | Cmd::Dock { .. }
        | Cmd::Split { .. }
        | Cmd::Focus { .. }
        | Cmd::Move { .. }
        | Cmd::Swap { .. }
        | Cmd::Zoom { .. }
        | Cmd::Resize { .. } => crate::layout::run(&mut c, cli)?,
        Cmd::Events { all } => crate::events::run(c, cli, *all, &server)?,
        Cmd::Api(cmd) => crate::api::run(&mut c, cli, cmd)?,
    }
    Ok(())
}

/// The commands about this machine, which refuse -H: their name.
fn local_only(cmd: &Cmd) -> Option<&'static str> {
    Some(match cmd {
        Cmd::App => "app",
        Cmd::Server(ServerCmd::Run { .. }) => "server run",
        Cmd::Server(ServerCmd::Start { .. }) => "server start",
        Cmd::Server(ServerCmd::Proxy) => "server proxy",
        Cmd::Config(ConfigCmd::Check { .. }) => "config check",
        Cmd::Config(ConfigCmd::Get { .. }) => "config get",
        Cmd::Config(ConfigCmd::Set { .. }) => "config set",
        Cmd::Theme(_) => "theme",
        Cmd::Skill(_) => "skill",
        Cmd::Hosts(_) => "hosts",
        Cmd::SelfUpdate { .. } => "self-update",
        _ => return None,
    })
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
    if cfg.setting_sources.contains_key("resume") {
        println!("resume: {}", cfg.settings.resume.as_str());
    }
    for d in &cfg.diagnostics {
        println!("{d}");
    }
    for l in &cfg.logs {
        println!("{}: log.{}: {}", l.at, l.level.as_str(), l.message);
    }
    let shader = !cfg.settings.cursor_shader.trim().is_empty();
    let shader_failed = shader && !check_shader(&cfg.settings.cursor_shader, &path);
    match cfg.diagnostics.iter().filter(|d| d.level == leon_lua::Level::Error).count() + shader_failed as usize {
        0 => Ok(()),
        errors => Err(Fail::Other(format!("{} in {}", n(errors, "error"), path.display()))),
    }
}

/// Translate the `cursor_shader` file as Leon.app will and say how it went; false if it fails.
#[cfg(target_os = "macos")]
fn check_shader(setting: &str, config: &std::path::Path) -> bool {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let Some(file) = leon_lua::settings::shader_path(setting, config, home.as_deref()) else { return true };
    match leon_app::cursor_shader::load(&file) {
        Ok(_) => println!("cursor_shader: {}: ok", file.display()),
        Err(e) => {
            println!("cursor_shader: {e}");
            return false;
        }
    }
    true
}

/// Off macOS the setting is kept and shown, not drawn.
#[cfg(not(target_os = "macos"))]
fn check_shader(_: &str, _: &std::path::Path) -> bool {
    println!("cursor_shader: cursor shaders are macOS only for now");
    true
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

/// What a failure says, for the alert a Finder launch shows instead of stderr.
pub(crate) fn message(f: &Fail) -> String {
    match f {
        Fail::Api(e) => e.to_string(),
        Fail::Unreachable(m) | Fail::Other(m) | Fail::Usage(m) => m.clone(),
        Fail::Exit(code) => format!("exit status {code}"),
    }
}

pub(crate) fn resolve(c: &mut Client, kind: &str, query: &str, session_id: Option<&str>) -> Result<String, Fail> {
    let r = c.call("target.resolve", json!({ "kind": kind, "query": query, "session_id": session_id }))?;
    Ok(r["id"].as_str().unwrap_or_default().to_string())
}

/// The caller's own session: the one holding `$LEON_BLOCK` now, else `$LEON_SESSION`. A pane can
/// join another session (`session.move_block`) while its program keeps the env it started with.
pub(crate) fn own_session(c: &mut Client) -> Option<String> {
    let var = |n| std::env::var(n).ok().filter(|s: &String| !s.is_empty());
    let held = var("LEON_BLOCK").and_then(|b| c.call("target.resolve", json!({ "kind": "block", "query": b })).ok());
    held.and_then(|r| r["session_id"].as_str().map(String::from)).or_else(|| var("LEON_SESSION"))
}

/// The session a command acts on: an explicit target, then `-s`, then your own (`own_session`),
/// then the only session if there is exactly one.
pub(crate) fn pick_session(c: &mut Client, cli: &Cli, explicit: Option<&str>) -> Result<String, Fail> {
    find_session(c, cli, explicit)?
        .ok_or_else(|| Fail::Api(ApiError::not_found("which session? pass -s or set LEON_SESSION")))
}

/// `pick_session`, but None when nothing names a session and there isn't exactly one.
pub(crate) fn find_session(c: &mut Client, cli: &Cli, explicit: Option<&str>) -> Result<Option<String>, Fail> {
    let own = match explicit.or(cli.session.as_deref()) {
        Some(_) => None,
        None => own_session(c),
    };
    if let Some(q) = explicit.or(cli.session.as_deref()).or(own.as_deref()) {
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

pub(crate) fn pretty(v: &Value) -> String {
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

/// A new block's directory: `--cwd` against this one, else this one. With `-H` this machine's
/// directories mean nothing there: `--cwd` must be absolute, and without it the host's server
/// starts the block in its home.
pub(crate) fn block_cwd(cli: &Cli, cwd: Option<&std::path::Path>) -> Result<Option<std::path::PathBuf>, Fail> {
    if cli.host.is_some() {
        return remote_cwd(cwd);
    }
    let here = std::env::current_dir().map_err(|e| Fail::Other(format!("no current directory: {e}")))?;
    Ok(Some(here.join(cwd.unwrap_or(".".as_ref()))))
}

fn remote_cwd(cwd: Option<&std::path::Path>) -> Result<Option<std::path::PathBuf>, Fail> {
    match cwd {
        Some(c) if c.is_relative() => {
            Err(Fail::Usage(format!("--cwd {}: with -H, give an absolute path on that machine", c.display())))
        }
        c => Ok(c.map(std::path::PathBuf::from)),
    }
}

/// Canonicalizing also resolves relative paths in the caller, before the server sees them.
fn new_directory(path: &std::path::Path) -> Result<std::path::PathBuf, Fail> {
    let dir = path
        .canonicalize()
        .map_err(|e| Fail::Usage(format!("--cwd {}: cannot open directory: {e}", path.display())))?;
    if !dir.is_dir() {
        return Err(Fail::Usage(format!("--cwd {}: not a directory", path.display())));
    }
    Ok(dir)
}

/// The `--help` footer.
pub(crate) const EXIT_CODES: &str = "\
Exit codes:
  0    ok
  1    failed
  2    bad command line
  3    target not found or ambiguous
  4    server unreachable
  10   `self-update --check`: a newer release is available
  130  interrupted (Ctrl-C)
  `wait` and `run --wait` exit with the program's own status.";
