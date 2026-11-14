//! Layout commands: split, focus, move, swap, zoom, resize. CLI ratios are the new or moved
//! pane's share in percent; the API's `ratio` is the before side's share.

use clap::ValueEnum;
use leon_core::layout::{Direction, Side};
use leon_core::ApiError;
use serde_json::{json, Value};

use crate::blocks::pick_block;
use crate::client::{Client, Fail};
use crate::commands::{pick_session, print_json, resolve};
use crate::{Cli, Cmd, Edge};

/// (direction, side) for a pane going to `edge` of its anchor.
fn place(edge: Edge) -> (Direction, Side) {
    match edge {
        Edge::Right => (Direction::Horizontal, Side::After),
        Edge::Left => (Direction::Horizontal, Side::Before),
        Edge::Above => (Direction::Vertical, Side::Before),
        Edge::Below => (Direction::Vertical, Side::After),
    }
}

/// The API ratio that gives the pane on `side` `pct` percent.
fn ratio(side: Side, pct: u8) -> f64 {
    f64::from(if side == Side::Before { pct } else { 100 - pct }) / 100.0
}

pub fn run(c: &mut Client, cli: &Cli) -> Result<(), Fail> {
    let r = match &cli.cmd {
        Cmd::Split { edge, ratio: pct, label, keep_open, cwd, focus, command } => {
            let (sid, anchor) = match cli.block.as_deref().filter(|b| b.starts_with("split:")) {
                Some(split) => (pick_session(c, cli, None)?, split.to_string()),
                None => pick_block(c, cli, None)?,
            };
            let here = std::env::current_dir().map_err(|e| Fail::Other(format!("no current directory: {e}")))?;
            let cwd = here.join(cwd.as_deref().unwrap_or(".".as_ref()));
            let command = (!command.is_empty()).then_some(command);
            let options = json!({ "command": command, "cwd": cwd, "exit": { "on_completion": !keep_open } });
            let (direction, side) = place(*edge);
            let r = c.call(
                "session.new_split",
                json!({
                    "session_id": sid, "anchor_id": anchor, "direction": direction, "side": side,
                    "ratio": ratio(side, *pct), "focus": focus,
                    "layout": { "block": { "label": label, "options": options } },
                }),
            )?;
            if !cli.json {
                println!("{}", r["block_id"].as_str().unwrap_or_default());
            }
            r
        }
        Cmd::Focus { target, wrap } => match target.as_str() {
            "left" | "right" | "up" | "down" => {
                let sid = pick_session(c, cli, None)?;
                c.call("session.focus_direction", json!({ "session_id": sid, "direction": target, "wrap": wrap }))?
            }
            "next" | "prev" => {
                let sid = pick_session(c, cli, None)?;
                let m = if target == "next" { "session.focus_next_pane" } else { "session.focus_previous_pane" };
                c.call(m, json!({ "session_id": sid, "wrap": wrap }))?
            }
            _ => {
                let (sid, bid) = pick_block(c, cli, Some(target))?;
                c.call("session.focus_block", json!({ "session_id": sid, "block_id": bid }))?
            }
        },
        Cmd::Move { args, ratio: pct } => {
            let (block, edge, anchor) = match args.as_slice() {
                [b, e, a] => (Some(b.as_str()), e, a),
                [e, a] => (None, e, a),
                _ => unreachable!("clap takes 2 or 3"),
            };
            let edge = Edge::from_str(edge, true).map_err(|_| bad("DIRECTION is right, left, above or below"))?;
            let (sid, bid) = pick_block(c, cli, block)?;
            let anchor = resolve(c, "block", anchor, Some(&sid))?;
            let (direction, side) = place(edge);
            c.call(
                "session.move_block",
                json!({
                    "session_id": sid, "block_id": bid, "anchor_block_id": anchor,
                    "direction": direction, "side": side, "ratio": ratio(side, *pct),
                }),
            )?
        }
        Cmd::Swap { block, other } => {
            let (sid, bid) = pick_block(c, cli, Some(block))?;
            let other = resolve(c, "block", other, Some(&sid))?;
            c.call("session.swap_blocks", json!({ "session_id": sid, "block_id": bid, "other_block_id": other }))?
        }
        Cmd::Zoom { block, on, off } => {
            let (sid, bid) = pick_block(c, cli, block.as_deref())?;
            let zoom = if *on {
                json!(true)
            } else if *off {
                json!(false)
            } else {
                json!("toggle")
            };
            c.call("session.zoom_block", json!({ "session_id": sid, "block_id": bid, "zoom": zoom }))?
        }
        Cmd::Resize { block, ratio: pct } => {
            let (sid, bid) = pick_block(c, cli, block.as_deref())?;
            let view = c.call("session.view", json!({ "session_id": sid }))?;
            let windows = view["windows"].as_array().cloned().unwrap_or_default();
            let split = windows.iter().find_map(|w| parent(&w["layout"], &bid));
            let split = split.ok_or_else(|| bad("that block isn't in a split"))?;
            let side = if leaf(&split["before"]) == Some(bid.as_str()) { Side::Before } else { Side::After };
            c.call(
                "session.resize_pane",
                json!({
                    "session_id": sid, "direction": split["direction"],
                    "before_block_id": first_leaf(&split["before"]), "after_block_id": first_leaf(&split["after"]),
                    "ratio": ratio(side, *pct),
                }),
            )?
        }
        _ => unreachable!("commands::run routes only layout commands here"),
    };
    if cli.json {
        print_json(&r);
    }
    Ok(())
}

fn bad(message: &str) -> Fail {
    Fail::Api(ApiError::bad_request(message))
}

/// The block id of a leaf node.
fn leaf(node: &Value) -> Option<&str> {
    node["block"]["block_id"].as_str()
}

fn first_leaf(node: &Value) -> &str {
    match (leaf(node), node.get("split")) {
        (Some(id), _) => id,
        (None, Some(s)) => first_leaf(&s["before"]),
        (None, None) => "",
    }
}

/// The split (`{split_id, direction, ratio, before, after}`) that has block `id` as a direct child.
fn parent<'a>(node: &'a Value, id: &str) -> Option<&'a Value> {
    let s = node.get("split")?;
    if leaf(&s["before"]) == Some(id) || leaf(&s["after"]) == Some(id) {
        return Some(s);
    }
    parent(&s["before"], id).or_else(|| parent(&s["after"], id))
}
