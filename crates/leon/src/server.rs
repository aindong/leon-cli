//! `leon server …` and `leon app`: starting, running, stopping and proxying the server, and
//! opening the app on it.

use std::time::{Duration, Instant};

use leon_core::paths::socket_path;
use serde_json::{json, Value};

use crate::client::{spawn_server, Client, Fail};
use crate::{Cli, ServerCmd};

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

#[cfg(all(feature = "app", target_os = "macos"))]
pub(crate) fn app(cli: &Cli, socket: &std::path::Path) -> Result<(), Fail> {
    // the app is a client: the server comes first, as `leon server start` makes it
    start_server(socket, true, cli.autostart).inspect_err(|e| leon_app::alert(&crate::commands::message(e)))?;
    leon_app::run(socket, env!("LEON_VERSION")).map_err(Fail::Other)
}

/// A headless build (Linux, or `--no-default-features`): `leon` alone is `leon attach` in a
/// terminal, and prints the help anywhere else (a pipe or a script, where attach can't run).
#[cfg(not(all(feature = "app", target_os = "macos")))]
pub(crate) fn app(cli: &Cli, _socket: &std::path::Path) -> Result<(), Fail> {
    use std::io::IsTerminal;
    if cli.cmd.is_some() {
        return Err(Fail::Other("this leon was built without the app (the `app` feature, macOS only for now)".into()));
    }
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        let server = crate::hosts::server(cli, cli.autostart)?;
        return crate::attach::run(cli.session.as_deref(), None, cli.autostart, &server);
    }
    if cli.session.is_some() || cli.host.is_some() {
        return Err(Fail::Usage("`leon` alone attaches only in a terminal; name a command".into()));
    }
    <Cli as clap::CommandFactory>::command().print_help().map_err(|e| Fail::Other(e.to_string()))
}

pub(crate) fn server(cli: &Cli, cmd: &ServerCmd) -> Result<(), Fail> {
    let socket = socket_path();
    match cmd {
        ServerCmd::Run { autostarted, restore, config } => {
            crate::login_path::adopt();
            let idle = std::env::var("LEON_IDLE_EXIT_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10);
            let info = leon_server::Info {
                pid: std::process::id(),
                version: env!("CARGO_PKG_VERSION"),
                socket: socket.display().to_string(),
            };
            let ctx = std::sync::Arc::new(leon_server::Ctx::new(info, *autostarted));
            let state_dir = leon_core::paths::state_dir();
            let opts = leon_server::serve::Options { socket, idle_exit: Duration::from_secs(idle) };
            let rt = tokio::runtime::Runtime::new().map_err(|e| Fail::Other(e.to_string()))?;
            let serve = async {
                ctx.start_config(config.clone());
                let mut bound = leon_server::serve::listen(&opts.socket).await?;
                if let Some(dir) = state_dir {
                    // The config's setting, waited for briefly, and only if there is a restore to follow it.
                    let resume = if *restore { ctx.resume_policy().await } else { leon_lua::Resume::Never };
                    let p = leon_server::persist::Persist {
                        file: dir.join(leon_server::persist::FILE),
                        restore: *restore,
                        resume,
                    };
                    bound.persist(&ctx, &p); // after binding: a server that loses the race keeps nothing
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
        ServerCmd::Proxy => proxy(&socket, cli.autostart),
        ServerCmd::Stop { discard } => {
            let server = crate::hosts::server(cli, false)?;
            let mut c = server.connect(false)?;
            // Plain stops send no params, which servers from before `discard` also take.
            let params = if *discard { json!({ "discard": true }) } else { json!({}) };
            c.call("server.stop", params)?;
            if server.link.is_some() {
                return Ok(()); // a link always answers: polling it would only reach ssh
            }
            let end = Instant::now() + Duration::from_secs(5);
            while Instant::now() < end && Client::connect(&socket).is_ok() {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(())
        }
        ServerCmd::Status => {
            let mut c = crate::hosts::server(cli, false)?.connect(false)?;
            let st = c.call("server.status", json!({}))?;
            if cli.json {
                crate::commands::print_json(&st);
            } else {
                println!(
                    "running  pid {}  version {}  {} session(s)  {}",
                    st["pid"],
                    st["version"].as_str().unwrap_or(""),
                    st["sessions"],
                    st["socket"].as_str().unwrap_or("")
                );
                // the host's words, as the app shows them: no control characters
                let (t, text) = (&st["tailnet"], |v: &Value| leon_hosts::shown_text(v.as_str().unwrap_or(""), 300));
                match text(&t["state"]).as_str() {
                    "serving" => {
                        let at: Vec<String> = t["addresses"].as_array().into_iter().flatten().map(text).collect();
                        println!("tailnet  serving on {} for {}", at.join(", "), text(&t["login"]));
                    }
                    "stopped" => println!("tailnet  stopped: {}", text(&t["reason"])),
                    _ => {}
                }
                if let Some(r) = t["last_refusal"].as_object() {
                    println!("tailnet  last refused {}: {}", text(&r["peer"]), text(&r["reason"]));
                }
            }
            Ok(())
        }
    }
}

/// `leon server proxy`: this machine's server on stdin and stdout, for a client's link over ssh.
/// The server's peer-uid check passes because ssh runs this as the host's user.
fn proxy(socket: &std::path::Path, autostart: bool) -> Result<(), Fail> {
    if autostart {
        start_server(socket, true, true)?; // persistent: it outlives this ssh session
    }
    let dir = socket.parent().unwrap_or(std::path::Path::new("."));
    leon_core::paths::ensure_private_dir(dir).map_err(|e| Fail::Unreachable(e.to_string()))?;
    let stream = std::os::unix::net::UnixStream::connect(socket)
        .map_err(|e| Fail::Unreachable(format!("no leon server at {}: {e}", socket.display())))?;
    let mut up = stream.try_clone().map_err(|e| Fail::Other(e.to_string()))?;
    std::thread::spawn(move || {
        leon_hosts::pump(&mut std::io::stdin().lock(), &mut up);
        // the client is done: the server answers what it sent, then closes (its events park)
        let _ = up.shutdown(std::net::Shutdown::Write);
    });
    leon_hosts::pump(&mut &stream, &mut std::io::stdout().lock());
    Ok(())
}
