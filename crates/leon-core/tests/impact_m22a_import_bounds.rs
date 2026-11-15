//! M22a impact: theme files are untrusted bytes, so what an import does per input byte is bounded.
//! roxmltree 0.20 caps entity nesting (10 deep, 255 references under one top-level reference) but
//! not how many top-level references a file makes, so a file under 1 MiB could still expand to
//! gigabytes; entities are refused before parsing. Error messages clip what they echo.

use std::time::{Duration, Instant};

use leon_core::theme::import::parse;

#[test]
fn a_flood_of_top_level_entity_references_is_refused_at_once() {
    let a = "a".repeat(4000);
    let b = "&a;".repeat(250);
    let refs = "&b;".repeat(200_000); // 600 KB; expanded, 200k × 250 × 4 KB = 200 GB
    let xml = format!(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE plist [\n<!ENTITY a \"{a}\">\n<!ENTITY b \"{b}\">\n]>\n\
         <plist><dict><key>{refs}</key><dict/></dict></plist>\n"
    );
    assert!(xml.len() < 1 << 20);
    let start = Instant::now();
    let err = parse(xml.as_bytes()).unwrap_err();
    assert!(err.contains("entities"), "{err}");
    assert!(start.elapsed() < Duration::from_secs(2), "took {:?}", start.elapsed());
}

#[test]
fn errors_clip_what_they_echo() {
    let junk = "x".repeat(900_000);
    for input in [
        format!("background = #000000\n{junk}\n"),
        format!("background = {junk}\n"),
        format!("{junk} = #000000\n"),
        format!("<?xml version=\"1.0\"?><plist><dict><key>{junk}</key></dict></plist>"),
        format!("<?xml version=\"1.0\"?><{junk}/>"),
    ] {
        let err = parse(input.as_bytes()).unwrap_err();
        assert!(err.len() < 300, "a {} byte error: {}…", err.len(), &err[..80]);
    }
}
