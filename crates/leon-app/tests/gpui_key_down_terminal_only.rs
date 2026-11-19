//! GPUI phase 6 (ADR R9): `Workspace::key_down` is the terminal's path only. Each overlay takes
//! its keys from its own `on_key_down`, in view_overlays.rs: anywhere else in the app's sources,
//! an overlay's key handler is only defined, never called.
#[test]
fn overlays_take_their_keys_only_from_their_own_handlers() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut seen = 0;
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(e.path()).unwrap_or_default();
        for handler in ["palette_key(", "find_key(", "rename_key(", "overview_key("] {
            for line in text.lines().filter(|l| l.contains(handler) && !l.trim_start().starts_with("//")) {
                let defined = line.contains(&format!("fn {handler}"));
                if name == "view_overlays.rs" && !defined {
                    seen += 1;
                    continue;
                }
                assert!(defined, "{name} calls {handler}, which only its overlay's handler may: {}", line.trim());
            }
        }
    }
    assert!(seen >= 4, "view_overlays.rs calls each overlay's key handler: {seen}");
}
