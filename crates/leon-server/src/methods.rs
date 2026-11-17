//! Help, server, session and target methods (blocks are in blocks.rs).

use std::sync::atomic::Ordering;

use leon_core::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use leon_core::layout::Node;
use leon_core::State;

use crate::blocks::{self, BlockType, Layout};
use crate::contract::{self, Deprecated, Stability, API_VERSION};
use crate::Registry;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

#[derive(Serialize, JsonSchema)]
pub struct Empty {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionRef {
    pub session_id: String,
}

#[derive(Serialize, JsonSchema, Clone)]
pub struct MethodDoc {
    pub name: String,
    pub description: String,
    /// `stable` (covered by the compatibility policy), `experimental` or `internal`.
    pub stability: Stability,
    /// Present once the method is on its way out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    /// Example `params`, for the methods that have one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_example: Option<serde_json::Value>,
    /// The `result` that answers `request_example`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_example: Option<serde_json::Value>,
}

#[derive(Serialize, JsonSchema)]
pub struct Help {
    /// The API's semver (docs/integrators.md#compatibility), apart from the build's version.
    pub api_version: String,
    pub methods: Vec<MethodDoc>,
    /// Block creators, their flavors, and the block methods `block.method` calls.
    pub block_types: Vec<BlockType>,
}

#[derive(Serialize, JsonSchema)]
pub struct Status {
    /// Always `running` when the server answers.
    pub state: String,
    pub pid: u32,
    pub version: String,
    pub socket: String,
    /// True when a client started this server on demand; it exits once idle.
    pub autostarted: bool,
    pub sessions: usize,
    /// Where sessions are saved, while running and on stop; null when they aren't (the state dir isn't private).
    pub state_file: Option<String>,
    /// Serving over Tailscale (`serve_tailscale`): whether, where, for whom, and the last caller turned away.
    pub tailnet: crate::tailnet::TailnetStatus,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stop {
    /// Only stop when no session exists; otherwise fail with `conflict`.
    #[serde(default)]
    pub if_idle: bool,
    /// Don't save the sessions, and forget the ones saved before: the next start is empty.
    #[serde(default)]
    pub discard: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct Version {
    /// The build's version.
    pub version: String,
    /// The API's semver, as in `help.all`.
    pub api_version: String,
}

#[derive(Serialize, JsonSchema)]
pub struct SessionSummary {
    pub session_id: String,
    pub label: Option<String>,
    /// Number of windows (tabs).
    pub windows: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct SessionList {
    pub sessions: Vec<SessionSummary>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateSession {
    pub label: Option<String>,
    /// Label for the session's first window.
    pub window_label: Option<String>,
    /// Directory for the first terminal, overriding layout.block.options.cwd. Must exist;
    /// relative paths resolve against the server's cwd. Omitted preserves the layout default.
    pub cwd: Option<String>,
    /// What the first window holds; default a login shell.
    pub layout: Option<Layout>,
}

#[derive(Serialize, JsonSchema)]
pub struct Created {
    pub session_id: String,
    pub window_id: String,
    pub block_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetLabel {
    pub session_id: String,
    /// The new label; empty clears it.
    pub label: String,
}

#[derive(Serialize, JsonSchema)]
pub struct WindowView {
    pub window_id: String,
    pub label: Option<String>,
    /// 1-based position in tab order.
    pub position: usize,
    pub active: bool,
    /// In layout order.
    pub block_ids: Vec<String>,
    pub layout: Option<Node>,
    /// Back to front, including hidden floats.
    pub floating: Vec<leon_core::overlays::Floating>,
    /// At most one panel per side, including hidden drawers.
    pub docked: Vec<leon_core::overlays::Docked>,
    pub focused_block_id: Option<String>,
    /// The block shown filling the window; the layout is unchanged.
    pub zoomed_block_id: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct SessionView {
    pub session_id: String,
    pub label: Option<String>,
    /// Bumped by every change to this view.
    pub revision: u64,
    pub windows: Vec<WindowView>,
}

/// What a call that changes a session's view returns.
#[derive(Serialize, JsonSchema)]
pub struct Revision {
    pub revision: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewWindow {
    pub session_id: String,
    pub window_label: Option<String>,
    /// What the window holds; default a login shell.
    pub layout: Option<Layout>,
}

#[derive(Serialize, JsonSchema)]
pub struct WindowCreated {
    pub window_id: String,
    pub block_id: String,
    pub revision: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WindowRef {
    pub session_id: String,
    pub window_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetWindowLabel {
    pub session_id: String,
    pub window_id: String,
    pub label: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Session,
    Window,
    Block,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Resolve {
    pub kind: TargetKind,
    /// A full ID, a label (case-insensitive), a 1-based position, or an ID suffix of 4+ characters.
    pub query: String,
    /// Required for `window`: the session to look in. For `block`, omitted searches every session.
    pub session_id: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct Resolved {
    pub id: String,
    /// For a block: the session it belongs to.
    pub session_id: Option<String>,
}

pub(crate) fn view(s: &leon_core::Session) -> SessionView {
    SessionView {
        session_id: s.id.clone(),
        label: s.label.clone(),
        revision: s.revision,
        windows: s
            .windows
            .iter()
            .enumerate()
            .map(|(i, w)| WindowView {
                window_id: w.id.clone(),
                label: w.label.clone(),
                position: i + 1,
                active: i == s.active,
                block_ids: w
                    .layout
                    .as_ref()
                    .map_or(vec![], |t| t.leaves().into_iter().map(String::from).collect::<Vec<_>>())
                    .into_iter()
                    .chain(s.overlay(&w.id).ids().map(String::from))
                    .collect(),
                layout: w.layout.clone(),
                floating: s.overlay(&w.id).floating,
                docked: s.overlay(&w.id).docked,
                focused_block_id: w.focus.clone(),
                zoomed_block_id: w.zoomed.clone(),
            })
            .collect(),
    }
}

pub(crate) fn session_list(st: &State) -> Vec<SessionSummary> {
    st.sessions
        .iter()
        .map(|s| SessionSummary { session_id: s.id.clone(), label: s.label.clone(), windows: s.windows.len() })
        .collect()
}

pub(crate) fn revision(st: &State, session_id: &str) -> Revision {
    Revision { revision: st.revision(session_id) }
}

pub fn registry() -> Registry {
    let mut r = Registry::default();
    r.add("help.all", "Every method with its description and JSON Schemas.", |ctx, _: NoParams| {
        Ok(Help {
            api_version: API_VERSION.into(),
            methods: ctx
                .registry
                .methods()
                .iter()
                .map(|m| contract::doc(m.name, m.description, m.input_schema.clone(), m.output_schema.clone()))
                .collect(),
            block_types: blocks::block_types(),
        })
    });
    r.add(
        "server.status",
        "Whether the server runs, its pid, version and socket, and whether it serves over Tailscale.",
        |ctx, _: NoParams| {
            Ok(Status {
                state: "running".into(),
                pid: ctx.info.pid,
                version: ctx.info.version.into(),
                socket: ctx.info.socket.clone(),
                autostarted: ctx.is_autostarted(),
                sessions: ctx.state().sessions.len(),
                state_file: crate::events::lock(&ctx.state_file).as_ref().map(|f| f.display().to_string()),
                tailnet: ctx.tailnet.status(),
            })
        },
    );
    r.add("server.persist", "Keep running when idle (an autostarted server otherwise exits).", |ctx, _: NoParams| {
        ctx.autostarted.store(false, Ordering::SeqCst);
        Ok(Empty {})
    });
    r.add("server.version", "The server's version.", |ctx, _: NoParams| {
        Ok(Version { version: ctx.info.version.into(), api_version: API_VERSION.into() })
    });
    // The socket loop stops the server once this reply is written (serve.rs).
    r.add(
        "server.stop",
        "Stop the server after replying; every program ends. Sessions are saved unless `discard`.",
        |ctx, p: Stop| {
            let st = ctx.state(); // held while deciding, so no session is created in between
            if p.if_idle && !st.sessions.is_empty() {
                return Err(ApiError::new(409, "conflict", "sessions exist; not stopping"));
            }
            ctx.stopping.store(true, Ordering::SeqCst);
            ctx.discard.fetch_or(p.discard, Ordering::SeqCst); // any accepted stop that asked wins
            Ok(Empty {})
        },
    );
    r.add("session.list", "Every session in order, with its label and window count.", |ctx, _: NoParams| {
        Ok(SessionList { sessions: session_list(&ctx.state()) })
    });
    r.add("session.create", "Create a session with one window holding one terminal.", |ctx, p: CreateSession| {
        let mut st = ctx.state();
        if ctx.stopping.load(Ordering::SeqCst) {
            return Err(ApiError::new(503, "shutting_down", "the server is stopping"));
        }
        let mut layout = Layout::block(p.layout);
        if let Some(cwd) = p.cwd {
            if !std::path::Path::new(&cwd).is_dir() {
                return Err(ApiError::bad_request(format!("cwd {cwd:?}: not an existing directory")));
            }
            layout.options.cwd = Some(cwd);
        }
        let mark = st.mark();
        let (session_id, window_id) = st.create_session(p.label.as_deref(), p.window_label.as_deref())?;
        match blocks::open(ctx, &mut st, &session_id, &window_id, layout, None) {
            Ok(block_id) => Ok(Created { session_id, window_id, block_id }),
            Err(e) => {
                let _ = st.destroy_session(&session_id);
                st.forget_since(mark); // all or nothing, events included
                Err(e)
            }
        }
    });
    r.add("session.destroy", "Destroy a session and everything in it.", |ctx, p: SessionRef| {
        ctx.state().destroy_session(&p.session_id)?;
        Ok(Empty {})
    });
    r.add("session.set_label", "Rename a session.", |ctx, p: SetLabel| {
        let mut st = ctx.state();
        st.set_session_label(&p.session_id, &p.label)?;
        Ok(revision(&st, &p.session_id))
    });
    r.add(
        "session.view",
        "A session's windows in tab order, each with its layout tree, focused and zoomed block.",
        |ctx, p: SessionRef| Ok(view(ctx.state().session(&p.session_id)?)),
    );
    r.add(
        "session.new_window",
        "Add a window (tab) holding one terminal at the end of a session and make it active.",
        |ctx, p: NewWindow| {
            let mut st = ctx.state();
            let (mark, active) = (st.mark(), st.session(&p.session_id)?.active);
            let window_id = st.new_window(&p.session_id, p.window_label.as_deref())?;
            match blocks::open(ctx, &mut st, &p.session_id, &window_id, Layout::block(p.layout), None) {
                Ok(block_id) => {
                    Ok(WindowCreated { revision: revision(&st, &p.session_id).revision, window_id, block_id })
                }
                Err(e) => {
                    let _ = st.close_window(&p.session_id, &window_id);
                    st.forget_since(mark); // all or nothing, events included
                    st.session_mut(&p.session_id)?.active = active;
                    Err(e)
                }
            }
        },
    );
    r.add("session.close_window", "Close a window; closing the last one destroys the session.", |ctx, p: WindowRef| {
        let mut st = ctx.state();
        st.close_window(&p.session_id, &p.window_id)?;
        Ok(revision(&st, &p.session_id))
    });
    r.add("session.focus_window", "Make a window the session's active tab.", |ctx, p: WindowRef| {
        let mut st = ctx.state();
        st.focus_window(&p.session_id, &p.window_id)?;
        Ok(revision(&st, &p.session_id))
    });
    r.add("session.set_window_label", "Rename a window.", |ctx, p: SetWindowLabel| {
        let mut st = ctx.state();
        st.set_window_label(&p.session_id, &p.window_id, &p.label)?;
        Ok(revision(&st, &p.session_id))
    });
    r.add(
        "target.resolve",
        "Turn a query (ID, label, position or ID suffix) into one ID; ambiguous queries fail with every match.",
        |ctx, p: Resolve| {
            let st = ctx.state();
            Ok(match p.kind {
                TargetKind::Session => Resolved { id: st.resolve_session(&p.query)?.id.clone(), session_id: None },
                TargetKind::Window => {
                    let sid = p.session_id.ok_or_else(|| ApiError::bad_request("window targets need session_id"))?;
                    Resolved { id: st.resolve_window(&sid, &p.query)?.id.clone(), session_id: None }
                }
                TargetKind::Block => {
                    let (sid, id) = st.resolve_block(p.session_id.as_deref(), &p.query)?;
                    Resolved { id, session_id: Some(sid) }
                }
            })
        },
    );
    blocks::add(&mut r);
    crate::layout::add(&mut r);
    crate::overlays::add(&mut r);
    crate::events::add(&mut r);
    crate::resume::add(&mut r);
    crate::screens::add(&mut r);
    crate::lua::add(&mut r);
    r
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use crate::{Ctx, Info};

    fn ctx() -> Arc<Ctx> {
        Arc::new(Ctx::new(Info { pid: 1, version: "test", socket: "/tmp/x".into() }, false))
    }

    #[tokio::test]
    async fn help_lists_every_method_with_object_schemas() {
        let c = ctx();
        let help = c.call("help.all", json!(null)).await.unwrap();
        let methods = help["methods"].as_array().unwrap();
        assert_eq!(methods.len(), c.registry.methods().len());
        for m in methods {
            assert_eq!(m["input_schema"]["type"], "object", "{}", m["name"]);
            assert!(!m["description"].as_str().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn sessions_through_the_registry() {
        let c = ctx();
        let created = c.call("session.create", json!({"label": "web"})).await.unwrap();
        let sid = created["session_id"].as_str().unwrap();
        let r = c.call("target.resolve", json!({"kind": "session", "query": "WEB"})).await.unwrap();
        assert_eq!(r["id"], sid);
        let w = c.call("session.new_window", json!({"session_id": sid, "window_label": "logs"})).await.unwrap();
        let v = c.call("session.view", json!({"session_id": sid})).await.unwrap();
        assert_eq!(v["windows"][1]["window_id"], w["window_id"]);
        assert_eq!(v["windows"][1]["active"], true);
    }

    #[tokio::test]
    async fn errors_are_typed() {
        let c = ctx();
        assert_eq!(c.call("nope", json!({})).await.unwrap_err().code, "method_not_found");
        assert_eq!(c.call("session.create", json!({"bogus": 1})).await.unwrap_err().code, "bad_request");
        assert_eq!(
            c.call("session.destroy", json!({"session_id": "session:x"})).await.unwrap_err().code,
            "session_not_found"
        );
    }
}
