//! Update criterion 23 (.agent/plans/2026-11-16-self-update-core.md): without `LEON_RELEASE_URL`
//! (what everyone runs) a release file comes from its `url` in the manifest, under the same
//! transport rule as every other request, and a file's `name` must be a plain file name.

mod release_kit;
mod update_kit;

use update_kit::{assert_refused, name, run, Install, Release, VERSION};

/// A jq filter that sets fields of this machine's CLI entry.
fn cli_entry(set: &str) -> String {
    format!("(.files[] | select(.name == \"{}\")) |= ({set})", name("cli"))
}

#[test]
fn a_file_is_fetched_from_the_manifests_url() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    // where the manifest says, which is not where the manifest itself is
    std::fs::create_dir(rel.dir.join("files")).unwrap();
    std::fs::rename(rel.dir.join(name("cli")), rel.dir.join("files/the-cli")).unwrap();
    rel.edit(&cli_entry(&format!(".url = \"{}/files/the-cli\"", srv.url())));
    let o = run(leon.cmd_at_address(&rel, &format!("{}/latest.json", srv.url())).arg("self-update"), 60);
    assert_eq!(o.code, 0, "the update goes through: {}", o.text());
    assert_eq!(srv.asked(), ["latest.json", "latest.json.minisig", "files/the-cli"]);
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
}

#[test]
fn a_manifest_url_of_plain_http_elsewhere_is_refused() {
    let rel = Release::new();
    rel.edit(&cli_entry(".url = \"http://192.0.2.1/leon.tar.gz\""));
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let o = run(leon.cmd_at_address(&rel, &format!("{}/latest.json", srv.url())).arg("self-update"), 10);
    assert_refused(&o, &["plain http", "http://192.0.2.1/leon.tar.gz"]);
    leon.assert_unchanged(before, "a manifest url of plain http");
    assert!(leon.curl_ran().iter().all(|c| !c.contains("192.0.2.1")), "curl never ran for it: {:?}", leon.curl_ran());
}

#[test]
fn a_file_name_that_is_a_path_is_refused() {
    for bad in ["../leon.tar.gz", "a/b.tar.gz", "/tmp/leon.tar.gz", ".", ""] {
        let rel = Release::new();
        rel.edit(&cli_entry(&format!(".name = \"{bad}\"")));
        let (srv, leon) = (rel.serve(), Install::cli());
        let before = leon.state();
        for o in [leon.update(&rel, &srv, &[]), leon.update(&rel, &srv, &["--check"])] {
            assert_refused(&o, &["name"]);
        }
        leon.assert_unchanged(before, "a file name that is a path");
        assert!(srv.asked().iter().all(|n| n.starts_with("latest.json")), "no file was fetched: {:?}", srv.asked());
    }
}
