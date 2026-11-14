//! Terminal blocks: opening one in a window, the `block.*` and `session.*_block*` methods, and
//! the terminal's own block methods (`dev.leon.terminal.*`, called through `block.method`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use leon_core::model::clean_label;
use leon_core::terminal::{Spec, Terminal};
use leon_core::{new_id, ApiError, Block, State};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::methods::{Empty, MethodDoc, SessionRef};
use crate::registry::{parse, schema, to_value};
use crate::{Ctx, Later, Registry};

pub const TERMINAL: &str = "dev.leon.terminal";
pub const SHELL_FLAVOR: &str = "dev.leon.terminal.shell";

/// What a new window holds. Today one block; splits come with M3.
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum Layout {
    Block(BlockSpec),
}

#[derive(Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct BlockSpec {
    /// Only `dev.leon.terminal.shell` exists (the default).
    pub flavor: Option<String>,
    pub label: Option<String>,
    #[serde(default)]
    pub options: TerminalOptions,
}

#[derive(Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct TerminalOptions {
    /// argv to run directly; omitted runs the login shell.
    pub command: Option<Vec<String>>,
    /// Working directory; defaults to `$HOME`.
    pub cwd: Option<String>,
    pub exit: Option<ExitOptions>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExitOptions {
    /// Close the block when its program exits (default true); false keeps it open.
    pub on_completion: bool,
}

impl Layout {
    pub fn block(layout: Option<Layout>) -> BlockSpec {
        layout.map(|Layout::Block(b)| b).unwrap_or_default()
    }
}

/// Start `spec` in window `window_id`. Call with the state lock held, so an instant exit can't
/// be reported before the block is in the model.
pub fn open(
    ctx: &Arc<Ctx>,
    st: &mut State,
    session_id: &str,
    window_id: &str,
    spec: BlockSpec,
) -> Result<String, ApiError> {
    if spec.flavor.as_deref().is_some_and(|f| f != SHELL_FLAVOR) {
        return Err(ApiError::bad_request(format!("unknown flavor; the only one is {SHELL_FLAVOR}")));
    }
    let label = clean_label(spec.label.as_deref())?;
    let o = spec.options;
    if o.command.as_ref().is_some_and(Vec::is_empty) {
        return Err(ApiError::bad_request("command needs at least a program name"));
    }
    let id = new_id("block");
    let env = [
        ("LEON_SESSION", session_id),
        ("LEON_BLOCK", id.as_str()),
        ("LEON_SERVER", ctx.info.socket.as_str()),
        ("TERM", "xterm-256color"),
    ];
    let spec = Spec {
        command: o.command.unwrap_or_default(),
        cwd: o.cwd.map(PathBuf::from),
        env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
    };
    let weak = Arc::downgrade(ctx);
    let block_id = id.clone();
    let terminal = Terminal::spawn(spec, move |code| {
        if let Some(ctx) = weak.upgrade() {
            ctx.state().block_exited(&block_id, code);
            ctx.exited.send_modify(|n| *n += 1);
        }
    })
    .map_err(|e| ApiError::bad_request(format!("can't start the program: {e}")))?;
    let keep_open = o.exit.is_some_and(|e| !e.on_completion);
    let block = Block {
        id: id.clone(),
        label,
        window_id: window_id.into(),
        keep_open,
        exit_code: None,
        terminal: Arc::new(terminal),
    };
    st.add_block(session_id, block)?;
    Ok(id)
}

#[derive(Serialize, JsonSchema)]
pub struct BlockView {
    pub block_id: String,
    pub label: Option<String>,
    pub window_id: String,
    pub pid: i32,
    /// The argv running (the login shell's path for a shell).
    pub command: Vec<String>,
    pub keep_open: bool,
    /// Set once the program exited (only seen on blocks kept open).
    pub exit_code: Option<i32>,
}

fn view(b: &Block) -> BlockView {
    BlockView {
        block_id: b.id.clone(),
        label: b.label.clone(),
        window_id: b.window_id.clone(),
        pid: b.terminal.pid(),
        command: b.terminal.command.clone(),
        keep_open: b.keep_open,
        exit_code: b.exit_code,
    }
}

#[derive(Serialize, JsonSchema)]
pub struct BlockList {
    pub blocks: Vec<BlockView>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockRef {
    pub session_id: String,
    pub block_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CallMethod {
    pub session_id: String,
    pub block_id: String,
    /// A block method such as `dev.leon.terminal.write`.
    pub method: String,
    /// The method's own arguments.
    pub args: Option<Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum InputEvent {
    /// Press a key given in binding syntax (`enter`, `ctrl+c`, `alt+left`, `a`).
    Key { key: String },
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SendEvent {
    pub session_id: String,
    pub block_id: String,
    pub event: InputEvent,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Wait {
    /// The block's full ID; works after the block closed, too.
    pub block_id: String,
    /// Give up after this long with error `timeout`; omitted waits for ever.
    pub timeout_ms: Option<u64>,
}

#[derive(Serialize, JsonSchema)]
pub struct Exited {
    pub exit_code: i32,
}

/// The live block `session_id`/`block_id` names.
fn terminal(ctx: &Ctx, r: &BlockRef) -> Result<Arc<Terminal>, ApiError> {
    let st = ctx.state();
    let s = st.session(&r.session_id)?;
    let b = s.blocks.iter().find(|b| b.id == r.block_id);
    Ok(b.ok_or_else(|| ApiError::not_found(format!("no block {} in {}", r.block_id, r.session_id)))?.terminal.clone())
}

pub fn add(r: &mut Registry) {
    r.add("session.list_blocks", "A session's blocks in creation order.", |ctx, p: SessionRef| {
        Ok(BlockList { blocks: ctx.state().session(&p.session_id)?.blocks.iter().map(view).collect() })
    });
    r.add("session.describe_block", "One block: its window, process and exit status.", |ctx, p: BlockRef| {
        let st = ctx.state();
        let (s, b) = st.block(&p.block_id)?;
        if s.id != p.session_id {
            return Err(ApiError::not_found(format!("no block {} in {}", p.block_id, p.session_id)));
        }
        Ok(view(b))
    });
    r.add("block.close", "Close a block and end its program; an emptied window closes too.", |ctx, p: BlockRef| {
        ctx.state().close_block(&p.session_id, &p.block_id)?;
        Ok(Empty {})
    });
    r.add("block.method", "Call a block method (see block_types in help.all) with `args`.", |ctx, p: CallMethod| {
        let t = terminal(ctx, &BlockRef { session_id: p.session_id, block_id: p.block_id })?;
        terminal_method(&t, &p.method, p.args.unwrap_or(Value::Null))
    });
    r.add("block.event", "Send an input event to a block, bypassing key bindings.", |ctx, p: SendEvent| {
        let t = terminal(ctx, &BlockRef { session_id: p.session_id, block_id: p.block_id })?;
        match p.event {
            InputEvent::Key { key } => t.key(&key).map_err(ApiError::bad_request)?,
        }
        Ok(Empty {})
    });
    r.add_deferred(
        "block.wait",
        "Wait for a terminal's program to exit and return its status (128 + signal if killed).",
        |ctx, p: Wait| {
            let mut exits = ctx.exited.subscribe(); // before the check, so no exit slips between
            let known = ctx.state().exit_status(&p.block_id);
            if known.is_none() {
                ctx.state().block(&p.block_id)?;
            }
            let ctx = ctx.clone();
            let id = p.block_id;
            let wait = async move {
                loop {
                    if let Some(exit_code) = ctx.state().exit_status(&id) {
                        return Ok(Exited { exit_code });
                    }
                    if exits.changed().await.is_err() {
                        return Err(ApiError::new(503, "shutting_down", "the server is stopping"));
                    }
                }
            };
            Ok(match p.timeout_ms {
                None => Box::pin(wait) as Later<Exited>,
                Some(ms) => Box::pin(async move {
                    tokio::time::timeout(Duration::from_millis(ms), wait)
                        .await
                        .unwrap_or_else(|_| Err(ApiError::new(408, "timeout", format!("still running after {ms} ms"))))
                }),
            })
        },
    );
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteArgs {
    /// Bytes to type, as UTF-8 text; nothing is added (no Enter).
    pub data: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FormatArgs {
    /// Only `text` so far.
    pub format: Option<String>,
    /// Drop trailing spaces and trailing blank lines.
    #[serde(default)]
    pub trim: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct Formatted {
    /// Scrollback then screen, oldest line first.
    pub text: String,
}

#[derive(Serialize, JsonSchema)]
pub struct TermSize {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Serialize, JsonSchema)]
pub struct Process {
    pub pid: i32,
    pub command: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

fn terminal_method(t: &Terminal, method: &str, args: Value) -> Result<Value, ApiError> {
    let args = if args.is_null() { Value::Object(Default::default()) } else { args };
    let name = method.strip_prefix(TERMINAL).and_then(|m| m.strip_prefix('.'));
    Ok(match name {
        Some("write") => {
            let a: WriteArgs = parse(method, args)?;
            t.write(a.data.as_bytes()).map_err(|e| ApiError::new(500, "io", e.to_string()))?;
            to_value(Empty {})
        }
        Some("format") => {
            let a: FormatArgs = parse(method, args)?;
            if a.format.as_deref().is_some_and(|f| f != "text") {
                return Err(ApiError::bad_request("only format \"text\" is supported"));
            }
            to_value(Formatted { text: t.text(a.trim) })
        }
        Some("size") => {
            let _: NoArgs = parse(method, args)?;
            let (rows, cols) = t.size();
            to_value(TermSize { rows, cols })
        }
        Some("process") => {
            let _: NoArgs = parse(method, args)?;
            to_value(Process { pid: t.pid(), command: t.command.clone() })
        }
        _ => return Err(ApiError::method_not_found(method)),
    })
}

#[derive(Serialize, JsonSchema)]
pub struct BlockType {
    pub creator: String,
    pub flavors: Vec<String>,
    pub methods: Vec<MethodDoc>,
}

pub fn block_types() -> Vec<BlockType> {
    fn doc<A: JsonSchema, R: JsonSchema>(name: &str, description: &str) -> MethodDoc {
        MethodDoc {
            name: format!("{TERMINAL}.{name}"),
            description: description.into(),
            input_schema: schema::<A>(),
            output_schema: schema::<R>(),
        }
    }
    vec![BlockType {
        creator: TERMINAL.into(),
        flavors: vec![SHELL_FLAVOR.into()],
        methods: vec![
            doc::<WriteArgs, Empty>("write", "Type text into the terminal."),
            doc::<FormatArgs, Formatted>("format", "The screen and scrollback as text."),
            doc::<NoArgs, TermSize>("size", "Rows and columns."),
            doc::<NoArgs, Process>("process", "The program's pid and argv."),
        ],
    }]
}
