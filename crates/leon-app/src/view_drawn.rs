//! What each view drew (tests): the debug selectors of its last render, which `view::drawn`
//! gathers for the tests to ask what is on screen (ADR R10).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use gpui::{Context, EntityId};

thread_local! {
    /// Per view, the debug selectors its last render drew. Only test builds name elements, and
    /// GPUI keeps a selector's bounds after its element is gone, so tests ask this what is on
    /// screen. A cached view GPUI reuses runs none of its `debug_selector`s, so each view's record
    /// is replaced only by its own render (ADR R10).
    static DRAWN: RefCell<HashMap<EntityId, HashSet<String>>> = RefCell::default();
    /// The view rendering now, whose record `sel` adds to.
    static DRAWING: Cell<Option<EntityId>> = const { Cell::new(None) };
}

/// Start over the record of what `cx`'s view draws: the first thing its render does. The record
/// goes when the view does.
pub fn drawing<V: 'static>(cx: &mut Context<V>) {
    let id = cx.entity_id();
    DRAWING.with(|d| d.set(Some(id)));
    if DRAWN.with(|d| d.borrow_mut().insert(id, HashSet::new()).is_none()) {
        cx.on_release(move |_, _| DRAWN.with(|d| drop(d.borrow_mut().remove(&id)))).detach();
    }
}

/// `name`, noted as drawn by the view rendering now: what each `debug_selector` closure returns.
pub fn sel(name: String) -> String {
    if let Some(id) = DRAWING.with(|d| d.get()) {
        DRAWN.with(|d| d.borrow_mut().entry(id).or_default().insert(name.clone()));
    }
    name
}

/// The debug selectors of what each view drew last (empty outside test builds).
pub fn drawn() -> HashSet<String> {
    DRAWN.with(|d| d.borrow().values().flatten().cloned().collect())
}
