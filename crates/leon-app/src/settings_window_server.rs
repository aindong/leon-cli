//! Settings › Server: This Mac's server from `server.status` (running and since when, its version,
//! socket, sessions, blocks and log), and Restart and Stop, each of which says first what happens
//! to the programs running in leon.

use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{PromptButton, PromptLevel};

use super::*;
use crate::view::Workspace;

/// This Mac's server as the Server page shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerInfo {
    pub version: String,
    pub socket: String,
    pub sessions: u64,
    /// None from a server older than API 1.10.0, as is `started_at`.
    pub blocks: Option<u64>,
    /// Seconds since the Unix epoch.
    pub started_at: Option<u64>,
    /// The file its output goes to, when it has one.
    pub log: Option<String>,
    /// It saves its sessions when it stops (it has a state file).
    pub saves: bool,
}

impl ServerInfo {
    /// From a `server.status` reply (an older server's too).
    pub fn parse(s: &Value) -> ServerInfo {
        let n = |k: &str| s[k].as_u64();
        let text = |k: &str| s[k].as_str().map(String::from);
        ServerInfo {
            version: text("version").unwrap_or_default(),
            socket: text("socket").unwrap_or_default(),
            sessions: n("sessions").unwrap_or(0),
            blocks: n("blocks"),
            started_at: n("started_at"),
            log: text("log"),
            saves: s["state_file"].is_string(),
        }
    }
}

/// What Stop (or with `restart`, Restart) asks before it acts, as (question, detail): what
/// `server.stop` does today, which depends on whether the server saves its sessions.
pub fn stop_warning(restart: bool, saves: bool) -> (String, String) {
    let ask = if restart { "Restart the leon server?" } else { "Stop the leon server?" };
    let then = match (restart, saves) {
        (false, true) => {
            "The sessions are saved with the text each pane shows, and come back when the server starts again."
        }
        (true, true) => {
            "leon starts again and brings the sessions back with the text each pane showed, each with a fresh \
             shell; a program that registered a resume command is offered it, as Resume programs after a restart says."
        }
        (false, false) => "This server doesn't save its sessions: they end for good.",
        (true, false) => "This server doesn't save its sessions: they end for good, and leon starts again with none.",
    };
    (ask.to_string(), format!("Every program in every session ends now. {then}"))
}

/// How long ago `started` was, from `now` (both in seconds since the epoch): `2 h 5 min`.
pub fn running_for(started: u64, now: u64) -> String {
    let s = now.saturating_sub(started);
    match (s / 86_400, s / 3600 % 24, s / 60 % 60) {
        (0, 0, 0) => "less than a minute".into(),
        (0, 0, m) => format!("{m} min"),
        (0, h, m) => format!("{h} h {m} min"),
        (d, h, _) => format!("{d} {} {h} h", if d == 1 { "day" } else { "days" }),
    }
}

impl SettingsWindow {
    /// What the page last read: the server, or why it didn't answer.
    pub fn server_info(&self) -> Option<Result<ServerInfo, String>> {
        self.server_info.clone()
    }

    /// The main window, when it is open.
    pub(super) fn workspace(cx: &App) -> Option<WindowHandle<Workspace>> {
        cx.windows().into_iter().find_map(|w| w.downcast::<Workspace>())
    }

    /// Ask This Mac's server how it is.
    pub(super) fn read_server(&mut self, cx: &mut Context<Self>) {
        let server = self.server.clone();
        cx.spawn(async move |this, cx| {
            let (tx, rx) = futures::channel::oneshot::channel();
            server.spawn("server.status", json!({}), move |r| drop(tx.send(r)));
            let read = match rx.await {
                Ok(Ok(s)) => Ok(ServerInfo::parse(&s)),
                Ok(Err(e)) => Err(e.message),
                Err(_) => Err("no answer".into()),
            };
            this.update(cx, |sw, cx| {
                sw.server_info = Some(read);
                cx.notify();
            })
        })
        .detach();
    }

    /// Ask, then stop the server (and with `restart`, start leon again once the main window has
    /// seen it gone, as its Start It Again does).
    fn confirm_stop(&mut self, restart: bool, window: &mut Window, cx: &mut Context<Self>) {
        let saves = matches!(&self.server_info, Some(Ok(i)) if i.saves);
        let (ask, detail) = stop_warning(restart, saves);
        let go = if restart { "Restart" } else { "Stop Server" };
        let buttons = [PromptButton::Ok(go.into()), PromptButton::Cancel("Cancel".into())];
        let answer = window.prompt(PromptLevel::Warning, &ask, Some(&detail), &buttons, cx);
        let server = self.server.clone();
        cx.spawn(async move |this, cx| {
            if answer.await != Ok(0) {
                return;
            }
            let (tx, rx) = futures::channel::oneshot::channel();
            server.spawn("server.stop", json!({}), move |r| drop(tx.send(r)));
            if let Ok(Err(e)) = rx.await {
                let _ = this.update(cx, |sw, cx| {
                    sw.error = Some(format!("can't stop the server: {}", e.message));
                    cx.notify();
                });
                return;
            }
            let _ = this.update(cx, |sw, cx| {
                if restart && Self::workspace(cx).is_none() {
                    sw.error = Some("The server stopped. leon's window is closed: open leon to start it again.".into());
                } else if restart {
                    sw.restarting = true;
                    sw.restart_when_stopped(cx);
                }
                sw.read_server(cx);
            });
            if restart {
                // a window that never sees the server go must not relaunch leon at some later stop
                cx.background_executor().timer(std::time::Duration::from_secs(10)).await;
                let _ = this.update(cx, |sw, _| sw.restarting = false);
            }
        })
        .detach();
    }

    /// Restart's second half: once the main window has seen the server gone (the reply to
    /// `server.stop` comes before it has saved and let go of its socket), start leon again as its
    /// Start It Again does. Called after the stop and on each change of the main window.
    pub(super) fn restart_when_stopped(&mut self, cx: &mut Context<Self>) {
        let Some(w) = self.restarting.then(|| Self::workspace(cx)).flatten() else { return };
        if w.read(cx).is_ok_and(|ws| ws.server_stopped()) {
            self.restarting = false;
            let _ = w.update(cx, |ws, _, cx| ws.start_again(cx));
        }
    }

    pub(super) fn server_page(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let shown = |p: &str| home_relative(Path::new(p), home.as_deref());
        let fact = |name: &str, value: String| {
            div()
                .flex()
                .gap(px(16.))
                .child(div().w(px(90.)).flex_none().text_color(rgb(t.muted)).child(name.to_string()))
                .child(div().flex_1().child(value))
        };
        let mut out = div().flex().flex_col().gap(px(10.));
        let info = match &self.server_info {
            None => return out.child(div().text_color(rgb(t.muted)).child("Asking the server…")),
            Some(Err(why)) => {
                return out.child(Self::label(
                    t,
                    "Not running",
                    &format!("This Mac's server didn't answer: {why}"),
                    true,
                ))
            }
            Some(Ok(i)) => i,
        };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        out = out
            .child(
                div()
                    .debug_selector(|| "server-running".into())
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Icon::Done.at(ROW_ICON, t.done))
                    .child(match info.started_at {
                        Some(at) => format!("Running for {}", running_for(at, now)),
                        None => "Running".into(),
                    }),
            )
            .child(fact("Version", info.version.clone()))
            .child(fact("Socket", shown(&info.socket)))
            .child(fact("Sessions", info.sessions.to_string()))
            .children(info.blocks.map(|b| fact("Blocks", b.to_string())));
        if let Some(log) = info.log.clone() {
            out = out.child(fact("Log", shown(&log)).child(text_button(
                t,
                "server-log".into(),
                "Reveal",
                cx,
                move |_, _, cx| cx.reveal_path(Path::new(&log)),
            )));
        }
        let mut actions = div().flex().gap(px(8.)).pt(px(6.));
        if Self::workspace(cx).is_some() {
            actions = actions.child(text_button(t, "server-restart".into(), "Restart…", cx, |sw, window, cx| {
                sw.confirm_stop(true, window, cx)
            }));
        }
        actions = actions.child(text_button(t, "server-stop".into(), "Stop…", cx, |sw, window, cx| {
            sw.confirm_stop(false, window, cx)
        }));
        out.child(actions)
    }
}

/// A button with a word on it.
pub(super) fn text_button(
    t: &Theme,
    id: String,
    label: &'static str,
    cx: &mut Context<SettingsWindow>,
    run: impl Fn(&mut SettingsWindow, &mut Window, &mut Context<SettingsWindow>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(|| id)
        .flex_none()
        .px(px(10.))
        .py(px(4.))
        .rounded(px(6.))
        .border_1()
        .border_color(rgb(t.border))
        .cursor_pointer()
        .hover(|d| d.bg(rgb(t.hover)))
        .on_click(cx.listener(move |sw, _, window, cx| run(sw, window, cx)))
        .child(label)
}
