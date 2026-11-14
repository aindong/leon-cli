//! The M1 methods: help, server, session and target.

use std::sync::atomic::Ordering;

use leon_core::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, JsonSchema)]
pub struct MethodDoc {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
}

#[derive(Serialize, JsonSchema)]
pub struct Help {
    pub methods: Vec<MethodDoc>,
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
}

#[derive(Serialize, JsonSchema)]
pub struct Version {
    pub version: String,
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
}

#[derive(Serialize, JsonSchema)]
pub struct Created {
    pub session_id: String,
    pub window_id: String,
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
}

#[derive(Serialize, JsonSchema)]
pub struct SessionView {
    pub session_id: String,
    pub label: Option<String>,
    pub windows: Vec<WindowView>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewWindow {
    pub session_id: String,
    pub window_label: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct WindowCreated {
    pub window_id: String,
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
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Resolve {
    pub kind: TargetKind,
    /// A full ID, a label (case-insensitive), a 1-based position, or an ID suffix of 4+ characters.
    pub query: String,
    /// Required for `window`: the session to look in.
    pub session_id: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct Resolved {
    pub id: String,
}

fn view(s: &leon_core::Session) -> SessionView {
    SessionView {
        session_id: s.id.clone(),
        label: s.label.clone(),
        windows: s
            .windows
            .iter()
            .enumerate()
            .map(|(i, w)| WindowView {
                window_id: w.id.clone(),
                label: w.label.clone(),
                position: i + 1,
                active: i == s.active,
            })
            .collect(),
    }
}

pub fn registry() -> Registry {
    let mut r = Registry::default();
    r.add("help.all", "Every method with its description and JSON Schemas.", |ctx, _: NoParams| {
        Ok(Help {
            methods: ctx
                .registry
                .methods()
                .iter()
                .map(|m| MethodDoc {
                    name: m.name.into(),
                    description: m.description.into(),
                    input_schema: m.input_schema.clone(),
                    output_schema: m.output_schema.clone(),
                })
                .collect(),
        })
    });
    r.add("server.status", "Whether the server runs, its pid, version and socket.", |ctx, _: NoParams| {
        Ok(Status {
            state: "running".into(),
            pid: ctx.info.pid,
            version: ctx.info.version.into(),
            socket: ctx.info.socket.clone(),
            autostarted: ctx.is_autostarted(),
            sessions: ctx.state().sessions.len(),
        })
    });
    r.add("server.persist", "Keep running when idle (an autostarted server otherwise exits).", |ctx, _: NoParams| {
        ctx.autostarted.store(false, Ordering::SeqCst);
        Ok(Empty {})
    });
    r.add("server.version", "The server's version.", |ctx, _: NoParams| {
        Ok(Version { version: ctx.info.version.into() })
    });
    // The socket loop stops the server once this reply is written (serve.rs).
    r.add("server.stop", "Stop the server after replying; every session ends.", |_, _: NoParams| Ok(Empty {}));
    r.add("session.list", "Every session in order, with its label and window count.", |ctx, _: NoParams| {
        Ok(SessionList {
            sessions: ctx
                .state()
                .sessions
                .iter()
                .map(|s| SessionSummary { session_id: s.id.clone(), label: s.label.clone(), windows: s.windows.len() })
                .collect(),
        })
    });
    r.add("session.create", "Create a session with one window.", |ctx, p: CreateSession| {
        let (session_id, window_id) = ctx.state().create_session(p.label.as_deref(), p.window_label.as_deref())?;
        Ok(Created { session_id, window_id })
    });
    r.add("session.destroy", "Destroy a session and everything in it.", |ctx, p: SessionRef| {
        ctx.state().destroy_session(&p.session_id)?;
        Ok(Empty {})
    });
    r.add("session.set_label", "Rename a session.", |ctx, p: SetLabel| {
        ctx.state().set_session_label(&p.session_id, &p.label)?;
        Ok(Empty {})
    });
    r.add("session.view", "A session's windows, in tab order.", |ctx, p: SessionRef| {
        Ok(view(ctx.state().session(&p.session_id)?))
    });
    r.add(
        "session.new_window",
        "Add a window (tab) at the end of a session and make it active.",
        |ctx, p: NewWindow| {
            Ok(WindowCreated { window_id: ctx.state().new_window(&p.session_id, p.window_label.as_deref())? })
        },
    );
    r.add("session.close_window", "Close a window; closing the last one destroys the session.", |ctx, p: WindowRef| {
        ctx.state().close_window(&p.session_id, &p.window_id)?;
        Ok(Empty {})
    });
    r.add("session.set_window_label", "Rename a window.", |ctx, p: SetWindowLabel| {
        ctx.state().set_window_label(&p.session_id, &p.window_id, &p.label)?;
        Ok(Empty {})
    });
    r.add(
        "target.resolve",
        "Turn a query (ID, label, position or ID suffix) into one ID; ambiguous queries fail with every match.",
        |ctx, p: Resolve| {
            let st = ctx.state();
            let id = match p.kind {
                TargetKind::Session => st.resolve_session(&p.query)?.id.clone(),
                TargetKind::Window => {
                    let sid = p.session_id.ok_or_else(|| ApiError::bad_request("window targets need session_id"))?;
                    st.resolve_window(&sid, &p.query)?.id.clone()
                }
            };
            Ok(Resolved { id })
        },
    );
    r
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{Ctx, Info};

    fn ctx() -> Ctx {
        Ctx::new(Info { pid: 1, version: "test", socket: "/tmp/x".into() }, false)
    }

    #[test]
    fn help_lists_every_method_with_object_schemas() {
        let c = ctx();
        let help = c.call("help.all", json!(null)).unwrap();
        let methods = help["methods"].as_array().unwrap();
        assert_eq!(methods.len(), c.registry.methods().len());
        for m in methods {
            assert_eq!(m["input_schema"]["type"], "object", "{}", m["name"]);
            assert!(!m["description"].as_str().unwrap().is_empty());
        }
    }

    #[test]
    fn sessions_through_the_registry() {
        let c = ctx();
        let created = c.call("session.create", json!({"label": "web"})).unwrap();
        let sid = created["session_id"].as_str().unwrap();
        let r = c.call("target.resolve", json!({"kind": "session", "query": "WEB"})).unwrap();
        assert_eq!(r["id"], sid);
        let w = c.call("session.new_window", json!({"session_id": sid, "window_label": "logs"})).unwrap();
        let v = c.call("session.view", json!({"session_id": sid})).unwrap();
        assert_eq!(v["windows"][1]["window_id"], w["window_id"]);
        assert_eq!(v["windows"][1]["active"], true);
    }

    #[test]
    fn errors_are_typed() {
        let c = ctx();
        assert_eq!(c.call("nope", json!({})).unwrap_err().code, "method_not_found");
        assert_eq!(c.call("session.create", json!({"bogus": 1})).unwrap_err().code, "bad_request");
        assert_eq!(
            c.call("session.destroy", json!({"session_id": "session:x"})).unwrap_err().code,
            "session_not_found"
        );
    }
}
