//! M10b criterion 2: the tab style, pane headers and sidebar width persist across app restarts.

mod m10b_support;

use std::os::unix::fs::PermissionsExt;

use gpui::TestAppContext;
use leon_app::chrome::{Settings, TabStyle};
use m10b_support::*;

#[test]
fn settings_round_trip_and_bad_files_give_the_defaults() {
    let dir = private_dir();
    let path = dir.path().join("app.json");
    assert_eq!(Settings::load(&path), Settings::default(), "a missing file is the defaults");
    let d = Settings::default();
    assert_eq!((d.tab_style, d.pane_headers, d.sidebar_width), (TabStyle::Sidebar, true, 220.));
    let mine = Settings { tab_style: TabStyle::Top, pane_headers: false, sidebar_width: 300. };
    mine.save(&path).unwrap();
    assert_eq!(Settings::load(&path), mine);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the file is private");
    std::fs::write(&path, "{ not json").unwrap();
    assert_eq!(Settings::load(&path), Settings::default(), "a corrupt file is the defaults");
    std::fs::write(&path, r#"{"tab_style":"top","sidebar_width":1e9}"#).unwrap();
    let odd = Settings::load(&path);
    assert_eq!(odd.tab_style, TabStyle::Top, "known fields are kept");
    assert!(odd.pane_headers, "a missing field takes its default");
    assert!(odd.sidebar_width <= 400., "an out-of-range width is clamped: {}", odd.sidebar_width);
}

#[test]
fn settings_are_not_saved_into_a_shared_dir() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let path = dir.path().join("app.json");
    assert!(Settings::default().save(&path).is_err(), "a world-writable dir is refused");
    assert!(!path.exists());
}

/// The real app's path: `view::run` builds its window with `Workspace::for_app`, which keeps
/// settings in `$LEON_STATE_DIR/app.json`. The only test in this binary that sets the variable.
#[gpui::test]
fn the_apps_own_workspace_keeps_settings_in_the_state_dir(cx: &mut TestAppContext) {
    let s = setup();
    let state = private_dir();
    std::env::set_var("LEON_STATE_DIR", state.path());
    let path = state.path().join("app.json");
    assert_eq!(leon_app::view::settings_path(), Some(path.clone()));
    let for_app = |cx: &mut TestAppContext| {
        cx.update(leon_app::view::init);
        let app = s.app.clone();
        let (ws, vcx) = cx.add_window_view(move |window, cx| {
            let remote = std::rc::Rc::new(leon_app::remote::Remote::connect(app.socket()).unwrap());
            leon_app::view::Workspace::for_app(remote, window, cx)
        });
        vcx.run_until_parked();
        (ws, vcx)
    };
    {
        let (ws, cx) = for_app(cx);
        act(&ws, cx, "client.tab_style.toggle");
        act(&ws, cx, "client.pane_header.reveal");
    }
    let saved = Settings::load(&path);
    assert_eq!((saved.tab_style, saved.pane_headers), (TabStyle::Top, false), "saved in the state dir");
    let (ws, cx) = for_app(cx);
    let got = ws.read_with(cx, |ws, _| ws.settings());
    assert_eq!((got.tab_style, got.pane_headers), (TabStyle::Top, false), "and read back on the next launch");
    std::env::remove_var("LEON_STATE_DIR");
    s.app.stop();
}

#[gpui::test]
fn a_new_workspace_starts_with_the_saved_tab_style(cx: &mut TestAppContext) {
    let s = setup();
    let dir = private_dir();
    let path = dir.path().join("app.json");
    {
        let (ws, cx) = open_with(cx, &s.app, Some(path.clone()));
        act(&ws, cx, "client.tab_style.toggle");
        act(&ws, cx, "client.pane_header.reveal");
    }
    assert!(path.exists(), "the toggle saved the settings");
    let (ws, cx) = open_with(cx, &s.app, Some(path.clone()));
    let got = ws.read_with(cx, |ws, _| ws.settings());
    assert_eq!((got.tab_style, got.pane_headers), (TabStyle::Top, false), "the next launch keeps them");
    assert!(drawn(cx, "tab-0"), "and draws tabs on top");
    s.app.stop();
}
