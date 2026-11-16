//! Update criterion 7 (.agent/plans/2026-11-16-self-update-core.md): fields the manifest gains
//! later are ignored (docs/releases.md's promise), and a schema other than 1 is refused with a way
//! out, since that leon cannot know what the manifest means.

mod release_kit;
mod update_kit;

use update_kit::{assert_refused, Install, Release, RELEASES, VERSION};

#[test]
fn unknown_fields_are_ignored() {
    let rel = Release::new();
    rel.edit(
        ".channel = \"stable\" | .extra = {a: [1, 2]} | .min_os.windows = \"11\" | .files[].mirror = \"elsewhere\"",
    );
    let (srv, leon) = (rel.serve(), Install::cli());
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "new fields don't stop an update: {}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
}

#[test]
fn another_schema_is_refused_with_a_way_out() {
    for filter in [".schema = 2", "{schema: 2, releases: []}", "del(.schema)", ".schema = \"1\""] {
        let rel = Release::new();
        rel.edit(filter);
        let (srv, leon) = (rel.serve(), Install::cli());
        let before = leon.state();
        for args in [&[][..], &["--check"]] {
            let o = leon.update(&rel, &srv, args);
            assert_refused(&o, &["this leon is too old to read this release; update by hand", RELEASES]);
        }
        leon.assert_unchanged(before, filter);
    }
}
