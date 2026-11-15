//! Brand: production art is the original thumbnail 11, scaled up. Its 280 px source is in the repo,
//! the 1024 master is built from it (brand/build-assets.cjs; brand/check-icon.cjs compares the
//! pixels), and the brand docs say so.
use std::path::Path;

fn repo(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)
}

/// A PNG's width, height and colour type, from its header.
fn header(path: &str) -> (u32, u32, u8) {
    let png = std::fs::read(repo(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(&png[1..4], b"PNG", "{path} is a PNG");
    (u32::from_be_bytes(png[16..20].try_into().unwrap()), u32::from_be_bytes(png[20..24].try_into().unwrap()), png[25])
}

#[test]
fn the_original_is_committed_and_the_master_keeps_its_colours() {
    let (w, h, _) = header("brand/icon/leon-original-280.png");
    assert_eq!((w, h), (280, 280), "the original thumbnail");
    // colour type 6 is RGBA: a 256-colour palette would dither the original's grain
    assert_eq!(header("brand/icon/leon-1024.png"), (1024, 1024, 6), "the master is 1024 x 1024 RGBA");
}

#[test]
fn the_brand_docs_say_production_is_the_original_scaled_up() {
    let read = |path: &str| std::fs::read_to_string(repo(path)).unwrap();
    let (readme, builder) = (read("brand/README.md"), read("brand/build-assets.cjs"));
    for (path, text) in [("brand/README.md", &readme), ("brand/build-assets.cjs", &builder)] {
        assert!(text.contains("leon-original-280.png"), "{path} names the original thumbnail");
        assert!(text.contains("Lanczos"), "{path} names the filter that scales it up");
        for stale in ["approved imagegen", "cleaned original", "approved raster", "never regenerates or overwrites"] {
            assert!(!text.contains(stale), "{path} still says {stale:?}");
        }
    }
    // the exact steps, with the numbers the check holds
    let geometry = "SCALE: 3, LEFT: 92, TOP: 94, TILE: { x: 106, y: 99, width: 812, height: 826, radius: 126 }";
    assert!(read("brand/check-icon.cjs").contains(geometry), "brand/check-icon.cjs holds the geometry");
    for step in ["check-icon.cjs", "3×", "(92, 94)", "812 × 826 rectangle at (106, 99)", "radius 126"] {
        assert!(readme.contains(step), "brand/README.md gives the step {step:?}");
    }
    assert!(readme.contains("the 16 × 16 px slot"), "brand/README.md names the one slot the optical icon fills");
}
