//! The title bar is one surface: its fill reaches the window's right edge past the last tab, or
//! the strip beside the tabs shows the desktop through a translucent window.

mod m10b_support;

use gpui::{px, TestAppContext};
use m10b_support::*;

#[gpui::test]
fn the_title_bar_fills_to_the_right_edge(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let right = |cx: &mut gpui::VisualTestContext| {
        let b = at(cx, "titlebar").expect("a title bar");
        let w = cx.update(|window, _| window.viewport_size().width);
        (b.right(), w)
    };
    act(&ws, cx, "client.tab_style.toggle"); // top tabs: the title bar shows beside the sidebar
    let (bar, window) = right(cx);
    assert!((bar - window).abs() < px(0.5), "beside the sidebar it ends at {bar:?}, not {window:?}");
    act(&ws, cx, "client.sidebar.toggle");
    let (bar, window) = right(cx);
    assert!((bar - window).abs() < px(0.5), "without the sidebar it ends at {bar:?}, not {window:?}");
    act(&ws, cx, "client.tab_style.toggle"); // sidebar style, sidebar hidden: the tab's name only
    let (bar, window) = right(cx);
    assert!((bar - window).abs() < px(0.5), "in sidebar style it ends at {bar:?}, not {window:?}");
    s.app.stop();
}
