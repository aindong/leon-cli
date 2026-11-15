//! Terminal blocks: opening one in a window, the `block.*` and `session.*_block*` methods, and
//! the terminal's own block methods (`dev.leon.terminal.*`, called through `block.method`).

use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use leon_core::model::clean_label;
use leon_core::mouse;
use leon_core::terminal::{Notice, Spec, Terminal, NAMESPACE as TERMINAL};
use leon_core::{new_id, ApiError, Block, Place, State};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::methods::{revision, Empty, MethodDoc, SessionRef};
use crate::registry::{parse, schema, to_value};
use crate::{Ctx, Later, Registry};

#[path = "blocks_view.rs"]
mod view;

pub const SHELL_FLAVOR: &str = "dev.leon.terminal.shell";

/// What a new window or split holds: one block (nested layouts aren't supported yet).
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

/// Start `spec` in window `window_id`, beside `place`'s anchor (None: as the empty window's only
/// block). Call with the state lock held, so an instant exit can't be reported before the block is
/// in the model.
pub fn open(
    ctx: &Arc<Ctx>,
    st: &mut State,
    session_id: &str,
    window_id: &str,
    spec: BlockSpec,
    place: Option<&Place>,
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
    let terminal = spawn(ctx, session_id, &id, o.command.unwrap_or_default(), o.cwd.map(PathBuf::from), &[])?;
    let keep_open = o.exit.is_some_and(|e| !e.on_completion);
    let block = Block {
        id: id.clone(),
        label,
        window_id: window_id.into(),
        keep_open,
        exit_code: None,
        terminal: Arc::new(terminal),
    };
    st.add_block(session_id, block, place)?;
    Ok(id)
}

/// Start the terminal of block `block_id`, showing `shown` first (`Terminal::spawn_showing`); an
/// empty `command` runs the login shell. Like `open`, call with the state lock held.
pub(crate) fn spawn(
    ctx: &Arc<Ctx>,
    session_id: &str,
    block_id: &str,
    command: Vec<String>,
    cwd: Option<PathBuf>,
    shown: &[u8],
) -> Result<Terminal, ApiError> {
    let mut env: Vec<(String, String)> = [
        ("LEON_SESSION", session_id),
        ("LEON_BLOCK", block_id),
        ("LEON_SERVER", ctx.info.socket.as_str()),
        ("TERM", "xterm-256color"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    // `leon` resolves in every terminal, even when the user's PATH lacks where it was installed
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)) {
        let path = std::env::var_os("PATH")
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "/usr/bin:/bin:/usr/sbin:/sbin".into());
        // no empty entries: one would mean the current directory
        let entries =
            std::iter::once(dir.clone()).chain(std::env::split_paths(&path).filter(|p| !p.as_os_str().is_empty()));
        // join_paths refuses a dir with ':' in it; then the terminal keeps the server's PATH
        let joined = std::env::join_paths(entries).ok().and_then(|p| p.into_string().ok());
        if let (Some(dir), Some(path)) = (dir.to_str(), joined) {
            env.extend([("LEON_BIN_DIR".to_string(), dir.to_string()), ("PATH".to_string(), path)]);
        }
    }
    let program = command.first().cloned();
    let spec = Spec { command, cwd, env };
    let weak = Arc::downgrade(ctx);
    let block_id = block_id.to_string();
    Terminal::spawn_showing(spec, shown, move |notice| {
        let Some(ctx) = weak.upgrade() else { return };
        let exited = matches!(notice, Notice::Exited(_));
        ctx.state().notice(&block_id, notice);
        if exited {
            ctx.exited.send_modify(|n| *n += 1);
        }
    })
    .map_err(|e| match program {
        // portable-pty's message lists every PATH entry it tried, kilobytes of them
        Some(p) if e.to_string().contains("No viable candidates found in PATH") => {
            ApiError::bad_request(format!("can't start the program: {p} isn't on PATH"))
        }
        _ => ApiError::bad_request(format!("can't start the program: {e}")),
    })
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
    /// The last title the program set (OSC 0/2).
    pub title: Option<String>,
    /// Where the program is: its cwd, else its last OSC 7 path, else where it started.
    pub cwd: Option<String>,
    /// What runs to resume the program after a restart, if it registered one (`set_resume`).
    pub resume: Option<Vec<String>>,
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
        title: b.terminal.title(),
        cwd: b.terminal.shown_cwd(),
        resume: b.terminal.resume(),
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
    /// The user arrived at the block: its `done` and `error` status records are cleared (reason `seen`).
    Focus,
    /// Paste text: line breaks become Enter, and if the program asked for bracketed paste the text
    /// is wrapped in its markers (with ESC and C1 controls removed), by the mode it has now.
    Paste { data: String },
    /// A mouse press, release or move on a screen cell (0-based), reported to the program in the
    /// mode and encoding it asked for now (X10, 1000, 1002, 1003; SGR, UTF-8 or legacy); nothing
    /// when it asked for none. `button` null is a move with no button down.
    Mouse {
        action: mouse::Action,
        button: Option<mouse::Button>,
        line: usize,
        col: usize,
        #[serde(default)]
        mods: mouse::Mods,
    },
    /// The wheel, `lines` notches up (down when negative, at most 100): reported as buttons 64/65,
    /// or on the alternate screen with no mouse mode as arrow keys.
    Wheel {
        lines: i32,
        line: usize,
        col: usize,
        #[serde(default)]
        mods: mouse::Mods,
    },
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
    r.add(
        "block.close",
        "Close a block and end its program; its split collapses, and an emptied window closes too.",
        |ctx, p: BlockRef| {
            let mut st = ctx.state();
            st.close_block(&p.session_id, &p.block_id)?;
            Ok(revision(&st, &p.session_id))
        },
    );
    r.add("block.method", "Call a block method (see block_types in help.all) with `args`.", |ctx, p: CallMethod| {
        let t = terminal(ctx, &BlockRef { session_id: p.session_id, block_id: p.block_id })?;
        terminal_method(&t, &p.method, p.args.unwrap_or(Value::Null))
    });
    r.add("block.event", "Send an input event to a block, bypassing key bindings.", |ctx, p: SendEvent| {
        let t = terminal(ctx, &BlockRef { session_id: p.session_id, block_id: p.block_id })?;
        match p.event {
            InputEvent::Key { key } => t.key(&key).map_err(ApiError::bad_request)?,
            InputEvent::Focus => t.acknowledge(), // the state lock is free here; the notice takes it
            InputEvent::Paste { data } => t.paste(&data).map_err(|e| ApiError::new(500, "io", e.to_string()))?,
            mouse @ (InputEvent::Mouse { .. } | InputEvent::Wheel { .. }) => view::mouse_event(&t, mouse)?,
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

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetResume {
    /// argv that resumes this program after a server restart, e.g. `["claude", "--resume", ID]`;
    /// null (or leaving it out) clears it. At most 64 arguments and 8 KiB of printable ASCII, no
    /// backslash.
    pub command: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportStatus {
    /// `working`, `blocked`, `done` or `error` (also `idle`, or `clear` to remove the record).
    pub state: String,
    /// One line of text, at most 192 bytes.
    pub title: Option<String>,
    /// One line of text, at most 2048 bytes.
    pub msg: Option<String>,
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
    /// Bytes of output the terminal has parsed (as the PTY delivered them: `\n` arrives as `\r\n`).
    pub output_bytes: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResizeArgs {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Serialize, JsonSchema)]
pub struct Pwd {
    /// The program's working directory, else the last OSC 7 path, else where it started; null if unknown.
    pub path: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ProgramStatus {
    /// `{id, record}` per record, least recently reported first; `id` null is the root record.
    /// A record has `state` and optional `kind`, `progress`, `app` (inherited), `title`, `msg`.
    pub records: Vec<Value>,
}

type TermFn = Box<dyn Fn(&Terminal, Value) -> Result<Value, ApiError> + Send + Sync>;

/// A terminal block method: its docs for `help.all` and its handler, kept in one table.
struct TermMethod {
    doc: MethodDoc,
    run: TermFn,
}

fn term_method<A, R>(name: &str, description: &str, f: fn(&Terminal, A) -> Result<R, ApiError>) -> TermMethod
where
    A: DeserializeOwned + JsonSchema + 'static,
    R: Serialize + JsonSchema + 'static,
{
    let full = format!("{TERMINAL}.{name}");
    let doc = crate::contract::doc(&full, description, schema::<A>(), schema::<R>());
    TermMethod { doc, run: Box::new(move |t, args| f(t, parse(&full, args)?).map(to_value)) }
}

static TERMINAL_METHODS: LazyLock<Vec<TermMethod>> = LazyLock::new(|| {
    let mut methods = vec![
        term_method("write", "Type text into the terminal.", |t, a: WriteArgs| {
            t.write(a.data.as_bytes()).map_err(|e| ApiError::new(500, "io", e.to_string()))?;
            Ok(Empty {})
        }),
        term_method("format", "The screen and scrollback as text.", |t, a: FormatArgs| {
            if a.format.as_deref().is_some_and(|f| f != "text") {
                return Err(ApiError::bad_request("only format \"text\" is supported"));
            }
            Ok(Formatted { text: t.text(a.trim) })
        }),
        term_method("size", "Rows and columns.", |t, _: NoArgs| {
            let (rows, cols) = t.size();
            Ok(TermSize { rows, cols })
        }),
        term_method(
            "resize",
            "Resize the grid and the PTY (at least 1 row, 2 columns); returns the new size.",
            |t, a: ResizeArgs| {
                t.resize(a.rows, a.cols).map_err(|e| ApiError::new(500, "io", e.to_string()))?;
                let (rows, cols) = t.size();
                Ok(TermSize { rows, cols })
            },
        ),
        term_method("pwd", "Where the program is: its working directory, else its last OSC 7 path.", |t, _: NoArgs| {
            Ok(Pwd { path: t.cwd().map(|p| p.display().to_string()) })
        }),
        term_method(
            "screen",
            "The screen as `block.watch` pushes it, whole: styled spans with RGB colours per row, \
             backgrounds, the cursor, and the input modes.",
            |t, _: NoArgs| {
                Ok(leon_core::screen::diff(None, &crate::screens::frame(t)).expect("a first screen is whole"))
            },
        ),
        term_method("process", "The program's pid, argv, and how much output was parsed.", |t, _: NoArgs| {
            Ok(Process { pid: t.pid(), command: t.command.clone(), output_bytes: t.output_bytes() })
        }),
        term_method(
            "set_resume",
            "Register the command that resumes this block's program after a server restart. Restored \
             blocks type it at the new prompt (setting `resume`: ask, always or never); nothing else \
             that ran is run again.",
            |t, a: SetResume| {
                if let Some(c) = &a.command {
                    crate::restore::check_command(c).map_err(ApiError::bad_request)?;
                }
                t.set_resume(a.command);
                Ok(Empty {})
            },
        ),
        term_method(
            "report_status",
            "Set the block's root program-status record as if its program had sent the OSC 7501 report \
             (`leon notify`), for tools that can't write one to the terminal. The spec's limits apply; text \
             OSC 7501 would drop (controls, line breaks, invisible characters) is a 400.",
            |t, a: ReportStatus| {
                t.report_status(&a.state, a.title.as_deref(), a.msg.as_deref()).map_err(ApiError::bad_request)?;
                Ok(Empty {})
            },
        ),
        term_method("program_status", "The OSC 7501 program-status records.", |t, _: NoArgs| {
            let records =
                t.program_status().into_iter().map(|(id, record)| serde_json::json!({ "id": id, "record": record }));
            Ok(ProgramStatus { records: records.collect() })
        }),
    ];
    methods.extend(view::methods());
    methods
});

fn terminal_method(t: &Terminal, method: &str, args: Value) -> Result<Value, ApiError> {
    let m = TERMINAL_METHODS.iter().find(|m| m.doc.name == method).ok_or_else(|| ApiError::method_not_found(method))?;
    (m.run)(t, args)
}

#[derive(Serialize, JsonSchema)]
pub struct BlockType {
    pub creator: String,
    pub flavors: Vec<String>,
    pub methods: Vec<MethodDoc>,
}

pub fn block_types() -> Vec<BlockType> {
    vec![BlockType {
        creator: TERMINAL.into(),
        flavors: vec![SHELL_FLAVOR.into()],
        methods: TERMINAL_METHODS.iter().map(|m| m.doc.clone()).collect(),
    }]
}
