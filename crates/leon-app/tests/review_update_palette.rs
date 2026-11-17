//! Review finding (.agent/plans/2026-11-16-app-update-notice.md, criterion 10): the update
//! notice's buttons are mouse-only, and the keys stay with the workspace. The palette offers each
//! button the notice has now, so the keyboard reaches all of them.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::updates::{How, Phase};
use m10b_support::*;

#[gpui::test]
fn the_palette_offers_what_the_notice_offers(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let rows = |ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.palette_rows_for("").into_iter().map(|(t, _)| t).collect::<Vec<_>>())
    };
    assert!(!rows(&ws, cx).iter().any(|t| t == "Dismiss the Update Notice"), "no notice, no rows for it");

    let brew = How::Brew("brew upgrade --cask aindong/leon/leon".into());
    let phase = Phase::Available { version: "9.9.9".into(), notes: "https://example.com/n".into(), how: brew };
    ws.read_with(cx, |ws, _| ws.updater().set_phase(phase));
    cx.run_until_parked();
    let now = rows(&ws, cx);
    for title in ["Copy the Update Command", "Open the Release Notes", "Dismiss the Update Notice"] {
        assert!(now.iter().any(|t| t == title), "{title}: {now:?}");
    }
    act(&ws, cx, "client.update.copy");
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
        Some("brew upgrade --cask aindong/leon/leon")
    );
    act(&ws, cx, "client.update.dismiss");
    assert!(!drawn(cx, "update-notice"), "dismissed from the palette");

    ws.read_with(cx, |ws, _| ws.updater().set_phase(Phase::Done { version: "9.9.9".into() }));
    cx.run_until_parked();
    assert!(rows(&ws, cx).iter().any(|t| t == "Restart to Finish the Update"));
    s.app.stop();
}
