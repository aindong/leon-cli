//! `session.new_split` and the layout methods: move, swap, zoom, resize, focus. Each returns the
//! session's `revision` (docs/spec.md §3).

use leon_core::layout::{Direction, Side, Toward};
use leon_core::panes::check_ratio;
use leon_core::{ApiError, Place};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::blocks::{self, Layout};
use crate::methods::{revision, Revision};
use crate::Registry;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewSplit {
    pub session_id: String,
    /// The block or split to split; or give `window_id` to split the window's whole layout.
    pub anchor_id: Option<String>,
    pub window_id: Option<String>,
    pub direction: Direction,
    /// Which side of the anchor the new block goes.
    pub side: Side,
    /// The before side's share (default 0.5).
    pub ratio: Option<f64>,
    pub layout: Layout,
    /// Focus the new block (default false).
    #[serde(default)]
    pub focus: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct SplitCreated {
    pub block_id: String,
    pub window_id: String,
    pub revision: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveBlock {
    pub session_id: String,
    pub block_id: String,
    /// The block to put it beside; any window of the session.
    pub anchor_block_id: String,
    /// Default horizontal.
    pub direction: Option<Direction>,
    /// Default after.
    pub side: Option<Side>,
    /// The before side's share (default 0.5).
    pub ratio: Option<f64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SwapBlocks {
    pub session_id: String,
    pub block_id: String,
    pub other_block_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Zoom {
    Set(bool),
    /// `"toggle"` (the default).
    Toggle(ZoomToggle),
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ZoomToggle {
    Toggle,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoomBlock {
    pub session_id: String,
    pub block_id: String,
    /// true, false or "toggle" (default).
    pub zoom: Option<Zoom>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResizePane {
    pub session_id: String,
    /// The direction of the split between the two blocks.
    pub direction: Direction,
    /// A block on the split's before side.
    pub before_block_id: String,
    /// A block on the split's after side.
    pub after_block_id: String,
    /// The before side's new share, between 0 and 1.
    pub ratio: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FocusBlock {
    pub session_id: String,
    pub block_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FocusDirection {
    pub session_id: String,
    pub direction: Toward,
    /// With nothing that way, go to the far side of the window.
    #[serde(default)]
    pub wrap: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FocusStep {
    pub session_id: String,
    /// Past the last block, go round to the first.
    #[serde(default)]
    pub wrap: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct Focused {
    pub block_id: String,
    pub revision: u64,
}

fn ratio(r: Option<f64>) -> Result<f64, ApiError> {
    let r = r.unwrap_or(0.5);
    check_ratio(r)?;
    Ok(r)
}

pub fn add(r: &mut Registry) {
    r.add(
        "session.new_split",
        "Split a block, a split, or a window's whole layout, and open a block beside it. All or nothing.",
        |ctx, p: NewSplit| {
            let ratio = ratio(p.ratio)?;
            let mut st = ctx.state();
            let (window_id, anchor) = match (p.anchor_id, p.window_id) {
                (Some(anchor), _) => (st.anchor_window(&p.session_id, &anchor)?, anchor),
                (None, Some(w)) => {
                    let s = st.session(&p.session_id)?;
                    let root = s.windows[s.window_index(&w)?].layout.as_ref().map(|t| t.id().to_string());
                    (w, root.ok_or_else(|| ApiError::bad_request("that window has no layout"))?)
                }
                (None, None) => return Err(ApiError::bad_request("give anchor_id or window_id")),
            };
            let place = Place { anchor, direction: p.direction, side: p.side, ratio, focus: p.focus };
            let block_id =
                blocks::open(ctx, &mut st, &p.session_id, &window_id, Layout::block(Some(p.layout)), Some(&place))?;
            Ok(SplitCreated { block_id, window_id, revision: revision(&st, &p.session_id).revision })
        },
    );
    r.add("session.move_block", "Put a block beside another, in any window of its session.", |ctx, p: MoveBlock| {
        let (direction, side) = (p.direction.unwrap_or(Direction::Horizontal), p.side.unwrap_or(Side::After));
        let ratio = ratio(p.ratio)?;
        let revision =
            ctx.state().move_block(&p.session_id, &p.block_id, &p.anchor_block_id, direction, side, ratio)?;
        Ok(Revision { revision })
    });
    r.add("session.swap_blocks", "Two blocks of a session trade places.", |ctx, p: SwapBlocks| {
        Ok(Revision { revision: ctx.state().swap_blocks(&p.session_id, &p.block_id, &p.other_block_id)? })
    });
    r.add("session.zoom_block", "Show one block filling its window (display only).", |ctx, p: ZoomBlock| {
        let zoom = match p.zoom {
            Some(Zoom::Set(on)) => Some(on),
            None | Some(Zoom::Toggle(_)) => None,
        };
        Ok(Revision { revision: ctx.state().zoom_block(&p.session_id, &p.block_id, zoom)? })
    });
    r.add("session.resize_pane", "Set the ratio of the split between two blocks.", |ctx, p: ResizePane| {
        let revision =
            ctx.state().resize_pane(&p.session_id, p.direction, &p.before_block_id, &p.after_block_id, p.ratio)?;
        Ok(Revision { revision })
    });
    r.add("session.focus_block", "Focus a block and make its window active.", |ctx, p: FocusBlock| {
        Ok(Revision { revision: ctx.state().focus_block(&p.session_id, &p.block_id)? })
    });
    r.add(
        "session.focus_direction",
        "Move focus left, right, up or down in the active window.",
        |ctx, p: FocusDirection| {
            let (revision, block_id) = ctx.state().focus_direction(&p.session_id, p.direction, p.wrap)?;
            Ok(Focused { block_id, revision })
        },
    );
    r.add("session.focus_next_pane", "Focus the next block of the active window.", |ctx, p: FocusStep| {
        let (revision, block_id) = ctx.state().focus_step(&p.session_id, true, p.wrap)?;
        Ok(Focused { block_id, revision })
    });
    r.add("session.focus_previous_pane", "Focus the previous block of the active window.", |ctx, p: FocusStep| {
        let (revision, block_id) = ctx.state().focus_step(&p.session_id, false, p.wrap)?;
        Ok(Focused { block_id, revision })
    });
}
