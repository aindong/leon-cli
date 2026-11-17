//! ADR R10 (docs/architecture/gpui.md): the drawn list is a record per view that only that view's
//! render replaces. A cached child GPUI reuses from the last frame runs none of its
//! `debug_selector`s, so a frame where only its parent rendered must keep the child's names.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{div, prelude::*, AnyView, Context, Entity, StyleRefinement, TestAppContext, Window};
use leon_app::view;

struct Child {
    renders: Rc<Cell<u32>>,
}

impl Render for Child {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view::drawing(cx);
        self.renders.set(self.renders.get() + 1);
        div().debug_selector(|| view::sel("child".into())).size_full()
    }
}

struct Root {
    name: String,
    child: Option<Entity<Child>>,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view::drawing(cx);
        let name = self.name.clone();
        let child = self.child.clone().map(|c| AnyView::from(c).cached(StyleRefinement::default().size_full()));
        div().debug_selector(|| view::sel(name)).size_full().children(child)
    }
}

#[gpui::test]
fn a_cached_view_keeps_its_names_across_a_frame_only_its_parent_drew(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let r = renders.clone();
    let (root, cx) =
        cx.add_window_view(move |_, cx| Root { name: "root-1".into(), child: Some(cx.new(|_| Child { renders: r })) });
    cx.run_until_parked();
    let drawn = view::drawn();
    assert!(drawn.contains("root-1") && drawn.contains("child"), "the first frame drew both: {drawn:?}");
    let before = renders.get();

    root.update(cx, |r, cx| {
        r.name = "root-2".into();
        cx.notify();
    });
    cx.run_until_parked();
    let drawn = view::drawn();
    assert_eq!(renders.get(), before, "the child was reused from its cache");
    assert!(drawn.contains("root-2"), "the root's new name: {drawn:?}");
    assert!(!drawn.contains("root-1"), "the root's render replaced its own record: {drawn:?}");
    assert!(drawn.contains("child"), "the cached child keeps its names: {drawn:?}");

    root.update(cx, |r, cx| {
        r.child = None;
        cx.notify();
    });
    cx.run_until_parked();
    let drawn = view::drawn();
    assert!(!drawn.contains("child"), "a dropped view's names go with it: {drawn:?}");
}
