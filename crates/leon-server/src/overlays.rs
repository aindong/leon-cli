//! Floating blocks and drawers. Validate before spawning; placement mutations hold one state lock.
use crate::blocks::{self, Layout};
use crate::layout::{FocusBlock, SplitCreated};
use crate::methods::{revision, Revision};
use crate::Registry;
use leon_core::layout::{Direction, Side};
use leon_core::overlays::{Bounds, DockSide};
use leon_core::{ApiError, Place, State};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Float {
    session_id: String,
    block_id: String,
    /// Fractions of the whole window; default centered 80% by 80%.
    bounds: Option<Bounds>,
    label: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewFloating {
    session_id: String,
    /// Default active window.
    window_id: Option<String>,
    bounds: Option<Bounds>,
    layout: Option<Layout>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Dock {
    session_id: String,
    block_id: String,
    side: DockSide,
    /// Fraction of the window (default 0.3), strictly between zero and one.
    size: Option<f64>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Toggle {
    session_id: String,
    /// Exact case-insensitive block label, unique within this session.
    label: String,
    /// When true and no block has this label, create a login-shell drawer at the bottom of the active window.
    #[serde(default)]
    create: bool,
}
fn placement(st: &State, sid: &str, wid: Option<&str>) -> Result<(String, Option<Place>), ApiError> {
    let s = st.session(sid)?;
    let w = match wid {
        Some(id) => &s.windows[s.window_index(id)?],
        None => &s.windows[s.active],
    };
    let p = w.layout.as_ref().map(|t| Place {
        anchor: t.id().into(),
        direction: Direction::Horizontal,
        side: Side::After,
        ratio: 0.5,
        focus: false,
    });
    Ok((w.id.clone(), p))
}
pub fn add(r: &mut Registry) {
    r.add(
        "session.float_block",
        "Move a block above its tiled layout; show, focus and raise it without restarting its program.",
        |ctx, p: Float| {
            let bounds = p.bounds.unwrap_or_default();
            bounds.check()?;
            let label = p.label.as_deref().map(|l| leon_core::model::clean_label(Some(l))).transpose()?;
            let mut st = ctx.state();
            st.float_block(&p.session_id, &p.block_id, bounds)?;
            if let Some(label) = label {
                let s = st.session_mut(&p.session_id)?;
                let i = s.block_index(&p.block_id)?;
                s.blocks[i].label = label;
            }
            Ok(revision(&st, &p.session_id))
        },
    );
    r.add(
        "session.new_floating",
        "Open a terminal in a visible floating pane in a window (default active).",
        |ctx, p: NewFloating| {
            let bounds = p.bounds.unwrap_or_default();
            bounds.check()?;
            let mut st = ctx.state();
            let (window_id, place) = placement(&st, &p.session_id, p.window_id.as_deref())?;
            let block_id =
                blocks::open(ctx, &mut st, &p.session_id, &window_id, Layout::block(p.layout), place.as_ref())?;
            let revision = st.float_block(&p.session_id, &block_id, bounds)?;
            Ok(SplitCreated { block_id, window_id, revision })
        },
    );
    r.add(
        "session.dock_block",
        "Pin a block to left, right or bottom; an occupied side fails without changes.",
        |ctx, p: Dock| {
            Ok(Revision {
                revision: ctx.state().dock_block(&p.session_id, &p.block_id, p.side, p.size.unwrap_or(0.3))?,
            })
        },
    );
    r.add(
        "session.unfloat_block",
        "Return a float or drawer to the tiled tree, split equally beside its root.",
        |ctx, p: FocusBlock| Ok(Revision { revision: ctx.state().unfloat_block(&p.session_id, &p.block_id)? }),
    );
    r.add(
        "session.toggle",
        "Show or hide a labeled float or drawer without stopping its program; optionally create a scratch shell.",
        |ctx, p: Toggle| {
            let mut st = ctx.state();
            if st.overlay_by_label(&p.session_id, &p.label)?.is_none() && p.create {
                let (wid, place) = placement(&st, &p.session_id, None)?;
                if st.session(&p.session_id)?.overlay(&wid).docked.iter().any(|d| d.side == DockSide::Bottom) {
                    return Err(ApiError::bad_request("bottom already has a dock"));
                }
                let spec = blocks::BlockSpec { label: Some(p.label), ..Default::default() };
                let id = blocks::open(ctx, &mut st, &p.session_id, &wid, spec, place.as_ref())?;
                st.dock_block(&p.session_id, &id, DockSide::Bottom, 0.3)?;
            } else {
                st.toggle_overlay(&p.session_id, &p.label)?;
            }
            Ok(revision(&st, &p.session_id))
        },
    );
}
