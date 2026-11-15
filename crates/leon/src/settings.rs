//! `leon config` and `leon theme`: the settings in init.lua and the themes they can name (leon's
//! built-in ones and the files in `themes/` beside init.lua). Apart from `config reload`, these
//! work on files here and need no server; `config set` reloads a running one so it follows.

use std::path::PathBuf;

use clap::Subcommand;
use leon_core::theme::{self, import, Theme};
use leon_lua::settings::{self, Settings, KEYS};
use serde_json::{json, Value};

use crate::client::Fail;
use crate::commands::print_json;
use crate::Cli;

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Load a config without applying it: print counts and diagnostics, exit 1 on error. Needs no server.
    Check {
        /// The config file [default: $LEON_CONFIG, else $XDG_CONFIG_HOME/leon/init.lua, else ~/.config/leon/init.lua].
        path: Option<std::path::PathBuf>,
    },
    /// Re-read the config on the server; a failed load keeps the last good one (exit 1).
    Reload,
    /// Print a setting from init.lua, or every setting without KEY. Needs no server.
    Get {
        /// theme, font.family, font.size, notifications.done|blocked|error, copy_on_select or resume.
        key: Option<String>,
    },
    /// Set a setting in init.lua, keeping the rest of the file, and reload a running server.
    Set {
        /// The setting, as `config get` names it.
        key: String,
        /// The value; for theme a NAME, or dark=NAME,light=NAME to follow the OS.
        value: String,
    },
}

#[derive(Subcommand)]
pub enum ThemeCmd {
    /// List the built-in and imported themes, and whether each is dark or light.
    List,
    /// Import a Ghostty theme file or an iTerm2 .itermcolors file into themes/ beside init.lua.
    Import {
        /// The theme file.
        file: PathBuf,
        /// The theme's name [default: the file's name, less .itermcolors or .theme].
        #[arg(long)]
        name: Option<String>,
    },
    /// Print a theme's colours, as a leon theme file.
    Show {
        /// The theme, as `theme list` names it.
        name: String,
    },
}

fn config_path() -> Result<PathBuf, Fail> {
    leon_lua::config_path().ok_or_else(|| Fail::Other("no config path: set LEON_CONFIG or HOME".into()))
}

pub fn theme(cli: &Cli, cmd: &ThemeCmd) -> Result<(), Fail> {
    let dir = settings::themes_dir(&config_path()?);
    match cmd {
        ThemeCmd::List => {
            let rows: Vec<Value> = settings::theme_names(&dir)
                .into_iter()
                .map(|name| match settings::load_theme(&dir, &name) {
                    Ok(t) => json!({ "name": name, "builtin": theme::builtin(&name).is_some(), "dark": t.is_dark() }),
                    Err(e) => json!({ "name": name, "builtin": false, "error": e }),
                })
                .collect();
            if cli.json {
                print_json(&Value::Array(rows));
                return Ok(());
            }
            for r in rows {
                let kind = match (&r["error"], r["dark"] == true) {
                    (Value::String(e), _) => format!("unreadable: {e}"),
                    (_, dark) => format!("{}{}", if dark { "dark" } else { "light" }, built_in(&r)),
                };
                println!("{}  {kind}", r["name"].as_str().unwrap_or_default());
            }
        }
        ThemeCmd::Import { file, name } => {
            let shown = file.display();
            let bytes = import::read_file(file).map_err(|e| Fail::Other(format!("can't read {shown}: {e}")))?;
            let t = import::parse(&bytes).map_err(|e| Fail::Other(format!("{shown}: {e}")))?;
            let name = match name {
                Some(n) => n.clone(),
                // Ghostty names have dots in them ("Hopscotch.256"): strip only a theme extension
                None => {
                    let n = file.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    [".itermcolors", ".theme"].iter().find_map(|x| n.strip_suffix(x)).unwrap_or(&n).to_string()
                }
            };
            settings::check_name(&name).map_err(Fail::Other)?;
            if theme::builtin(&name).is_some() {
                return Err(Fail::Other(format!("{name} is a built-in theme; pick another --name")));
            }
            let path = dir.join(format!("{name}.theme"));
            let err = |e: std::io::Error| Fail::Other(format!("can't write {}: {e}", path.display()));
            std::fs::create_dir_all(&dir).map_err(err)?;
            let created = std::fs::OpenOptions::new().write(true).create_new(true).open(&path);
            let mut f = match created {
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(Fail::Other(format!(
                        "a theme named {name} exists ({}); remove that file or pick another --name",
                        path.display()
                    )))
                }
                r => r.map_err(err)?,
            };
            std::io::Write::write_all(&mut f, import::to_text(&t).as_bytes()).map_err(err)?;
            println!("{name}: {}", path.display());
        }
        ThemeCmd::Show { name } => {
            let t = settings::load_theme(&dir, name).map_err(Fail::Other)?;
            if cli.json {
                print_json(&theme_json(t));
            } else {
                print!("{}", import::to_text(&t));
            }
        }
    }
    Ok(())
}

fn built_in(row: &Value) -> &'static str {
    if row["builtin"] == true {
        ", built in"
    } else {
        ""
    }
}

fn theme_json(mut t: Theme) -> Value {
    let hex = |c: u32| format!("#{c:06x}");
    let mut o = serde_json::Map::new();
    for key in ["background", "foreground", "cursor-color", "selection-background"].into_iter().chain(Theme::TOKENS) {
        o.insert(key.into(), json!(t.slot(key).map(|c| hex(*c))));
    }
    o.insert("palette".into(), json!(t.ansi.map(hex)));
    Value::Object(o)
}

/// `config get` and `config set`.
pub fn config(cli: &Cli, cmd: &ConfigCmd) -> Result<(), Fail> {
    let path = config_path()?;
    let unknown = |k: &str| Fail::Other(format!("unknown setting {k:?}; the settings are {}", KEYS.join(", ")));
    match cmd {
        ConfigCmd::Get { key } => {
            let cfg = if path.exists() {
                leon_lua::load(&path).map_err(|e| Fail::Other(e.to_string()))?
            } else {
                leon_lua::Config::default()
            };
            match key {
                Some(k) => {
                    let v = cfg.settings.get(k).ok_or_else(|| unknown(k))?;
                    if cli.json {
                        print_json(&v);
                    } else {
                        println!("{}", shown(&v));
                    }
                }
                None if cli.json => print_json(&cfg.settings.to_json()),
                None => {
                    for k in KEYS {
                        let at = cfg.setting_sources.get(k).map(|s| format!("  ({s})")).unwrap_or_default();
                        println!("{k} = {}{at}", cfg.settings.get(k).as_ref().map(shown).unwrap_or_default());
                    }
                }
            }
        }
        ConfigCmd::Set { key, value } => {
            let v = parse_value(key, value);
            let at = leon_lua::rewrite::write_setting(&path, key, &v).map_err(Fail::Other)?;
            println!("{key} = {} ({at})", shown(&v));
            // a running server follows; none is started for it
            let socket = leon_core::paths::socket_path();
            if let Ok(mut c) = crate::client::connect(&socket, false) {
                if let Err(Fail::Api(e)) = c.call("server.config_reload", json!({})) {
                    eprintln!("leon: warning: init.lua is written, but the server didn't reload it: {}", e.message);
                }
            }
        }
        ConfigCmd::Check { .. } | ConfigCmd::Reload => unreachable!("commands.rs runs these"),
    }
    Ok(())
}

/// The command line's VALUE as the setting's type: `true`, `15`, `dark=A,light=B`. Anything that
/// doesn't fit stays a string, for `Settings::set` to refuse with its own message.
fn parse_value(key: &str, text: &str) -> Value {
    match Settings::default().get(key) {
        Some(Value::Bool(_)) if text == "true" || text == "false" => Value::Bool(text == "true"),
        Some(Value::Number(_)) => text.trim().parse::<f64>().map_or_else(|_| text.into(), Value::from),
        Some(Value::Object(_)) if text.contains('=') => {
            let pairs: Option<serde_json::Map<String, Value>> = text
                .split(',')
                .map(|p| p.split_once('=').map(|(k, v)| (k.trim().to_string(), Value::from(v.trim()))))
                .collect();
            pairs.map_or_else(|| text.into(), Value::Object)
        }
        _ => text.into(),
    }
}

/// A setting as `config get` prints it: `15`, `leon-dark`, `dark=A,light=B`.
fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(_) => leon_lua::rewrite::literal(v),
        Value::Object(o) if o.get("dark").is_some() && o.get("dark") == o.get("light") => shown(&o["dark"]),
        Value::Object(o) => o.iter().map(|(k, v)| format!("{k}={}", shown(v))).collect::<Vec<_>>().join(","),
        v => v.to_string(),
    }
}
