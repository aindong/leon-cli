//! Criterion 13: the window's last frame is saved when the window is closed (the close button
//! quits the app through `on_window_closed`).

mod m10b_support;

use gpui::TestAppContext;
use leon_app::chrome::Settings;
use m10b_support::*;

#[gpui::test]
fn closing_the_window_saves_its_frame(cx: &mut TestAppContext) {
    let s = setup();
    let dir = private_dir();
    let path = dir.path().join("app.json");
    {
        // open_with resizes the window to 1400x900, which fires the bounds observer
        let (ws, vcx) = open_with(cx, &s.app, Some(path.clone()));
        drop(ws);
        // the close button: the window goes away first...
        vcx.update(|window, _| window.remove_window());
        vcx.run_until_parked();
    }
    // ...then the quit that `install`'s on_window_closed asks for
    cx.quit();
    let saved = Settings::load(&path).window;
    assert!(saved.is_some_and(|f| f.w == 1400. && f.h == 900.), "closing the window keeps its frame: {saved:?}");
    s.app.stop();
}
