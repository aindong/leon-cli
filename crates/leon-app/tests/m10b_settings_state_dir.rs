//! M10b criterion 2, the app's own path: settings in the state dir. Its own binary: it sets
//! LEON_STATE_DIR, before its server starts, which tests beside it in one binary must not see.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::chrome::{Settings, TabStyle};
use m10b_support::*;

/// The real app's path: `view::run` builds its window with `Workspace::for_app`, which keeps
/// settings in `$LEON_STATE_DIR/app.json`.
#[gpui::test]
fn the_apps_own_workspace_keeps_settings_in_the_state_dir(cx: &mut TestAppContext) {
    let state = private_dir();
    std::env::set_var("LEON_STATE_DIR", state.path()); // before the server starts
    let s = setup();
    let path = state.path().join("app.json");
    assert_eq!(leon_app::view::settings_path(), Some(path.clone()));
    {
        let (ws, cx) = for_app(cx, &s.app);
        act(&ws, cx, "client.tab_style.toggle");
        act(&ws, cx, "client.pane_header.reveal");
    }
    let saved = Settings::load(&path);
    assert_eq!((saved.tab_style, saved.pane_headers), (TabStyle::Top, false), "saved in the state dir");
    let (ws, cx) = for_app(cx, &s.app);
    let got = ws.read_with(cx, |ws, _| ws.settings());
    assert_eq!((got.tab_style, got.pane_headers), (TabStyle::Top, false), "and read back on the next launch");
    s.app.stop();
}

fn for_app<'a>(
    cx: &'a mut TestAppContext,
    app: &std::rc::Rc<leon_app::embedded::Embedded>,
) -> (gpui::Entity<leon_app::view::Workspace>, &'a mut gpui::VisualTestContext) {
    cx.update(leon_app::view::init);
    let app = app.clone();
    let (ws, vcx) = cx.add_window_view(move |window, cx| {
        let remote = std::rc::Rc::new(leon_app::remote::Remote::connect(app.socket()).unwrap());
        leon_app::view::Workspace::for_app(remote, window, cx)
    });
    vcx.run_until_parked();
    (ws, vcx)
}
