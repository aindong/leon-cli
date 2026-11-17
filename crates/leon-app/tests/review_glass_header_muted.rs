use leon_app::glass::{worst_contrast, Glass, SECONDARY};
use leon_app::theme::{contrast, DARK, LIGHT};
use leon_lua::settings::TRANSLUCENCY;

#[test]
fn a_pane_headers_muted_caption_keeps_secondary_contrast() {
    for (name, t) in [("leon-dark", DARK), ("leon-light", LIGHT)] {
        // fade_inactive_panes = 0: only fg sets the pane's alpha
        let g = Glass::new(&t, TRANSLUCENCY as f32, 0.);
        let need = SECONDARY.min(contrast(t.muted, t.bg));
        let muted = worst_contrast(t.muted, &[(t.bg, g.pane_alpha())]);
        assert!(muted >= need - 0.01, "{name}: muted header text on a pane {muted:.2} < {need:.2}");
    }
}
