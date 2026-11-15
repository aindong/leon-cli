//! Update criterion 16 (.agent/plans/2026-11-16-self-update-core.md): leon takes the manifest
//! entry whose kind, os and arch match how it is installed: `app` for a leon inside an app
//! bundle, `cli` otherwise. With no such entry it says so; it never guesses another.

mod release_kit;
mod update_kit;

use update_kit::{arch, assert_refused, name, name_for, os, Install, Release};

#[test]
fn a_release_without_this_platforms_cli_is_an_error_that_names_it() {
    let rel = Release::unsigned();
    std::fs::remove_file(rel.dir.join(name("cli"))).unwrap();
    rel.sign_args(&[]);
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    // --check too: a release this install cannot take is not "available"
    for args in [&[][..], &["--check"]] {
        let o = leon.update(&rel, &srv, args);
        assert_refused(&o, &[&format!("has no cli file for {} {}", os(), arch())]);
    }
    leon.assert_unchanged(before, "a release without this platform's file");
    assert!(srv.asked().iter().all(|n| n.starts_with("latest.json")), "no other file was tried: {:?}", srv.asked());
}

#[test]
fn a_leon_inside_an_app_bundle_asks_for_the_app() {
    // on Linux no release has an app; on macOS this one lost it
    let rel = Release::unsigned();
    let _ = std::fs::remove_file(rel.dir.join(name_for("app", "macos")));
    rel.sign_args(&[]);
    let (srv, leon) = (rel.serve(), Install::app());
    let before = leon.state();
    let o = leon.update(&rel, &srv, &[]);
    assert_refused(&o, &[&format!("has no app file for {} {}", os(), arch())]);
    leon.assert_unchanged(before, "a release without the app");
    assert!(srv.asked().iter().all(|n| n.starts_with("latest.json")), "the CLI tarball was not taken instead");
}

#[test]
fn a_leon_outside_a_bundle_takes_the_cli_tarball() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    assert_eq!(leon.update(&rel, &srv, &[]).code, 0);
    let files: Vec<String> = srv.asked().into_iter().filter(|n| !n.starts_with("latest.json")).collect();
    assert_eq!(files, [name("cli")], "exactly this machine's CLI tarball");
}

#[test]
fn an_app_that_is_not_leons_is_not_swapped_out() {
    // someone else's app that carries leon as a helper: a Leon.app in its place would delete it
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::at("Applications/Other.app/Contents/MacOS/leon"));
    let app = leon.home.join("Applications/Other.app");
    let plist = update_kit::INFO_PLIST.replace("dev.leon.app", "com.example.other");
    std::fs::write(app.join("Contents/Info.plist"), plist).unwrap();
    std::fs::create_dir(app.join("Contents/Resources")).unwrap();
    let theirs = app.join("Contents/Resources/theirs.txt");
    std::fs::write(&theirs, "keep me").unwrap();
    let before = leon.state();
    for args in [&[][..], &["--check"]] {
        let o = leon.update(&rel, &srv, args);
        assert_refused(&o, &[app.to_str().unwrap(), "is not a Leon.app", "will not replace another app"]);
    }
    assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "keep me");
    leon.assert_unchanged(before, "an app that is not leon's");
    assert_eq!(srv.requests(), Vec::<String>::new(), "and nothing was asked");
}

// macOS only: only macOS has an app to take
#[cfg(target_os = "macos")]
#[test]
fn a_leon_inside_a_bundle_takes_the_app_zip() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::app());
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "{}", o.text());
    let files: Vec<String> = srv.asked().into_iter().filter(|n| !n.starts_with("latest.json")).collect();
    assert_eq!(files, [name("app")], "exactly this machine's app zip");
    assert_eq!(leon.version(), format!("leon {} (fake)", update_kit::VERSION));
}
