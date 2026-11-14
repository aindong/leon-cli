//! M7b criterion 3 (.agent/plans/2026-11-14-m7b-ship.md): install.sh refuses to touch anything at
//! its paths that isn't a leon install, and changes nothing when it refuses.

mod install_kit;

use install_kit::Kit;

#[test]
fn a_foreign_cli_file_stops_the_install() {
    let k = Kit::new();
    std::fs::create_dir_all(k.cli().parent().unwrap()).unwrap();
    std::fs::write(k.cli(), "#!/bin/sh\necho someone else's leon\n").unwrap();
    let r = k.install(&[]);
    assert!(!r.ok, "install overwrote a foreign file: {}", r.out);
    assert!(r.out.contains(&*k.cli().display().to_string()), "names the foreign path: {}", r.out);
    assert_eq!(std::fs::read_to_string(k.cli()).unwrap(), "#!/bin/sh\necho someone else's leon\n");
    assert!(!k.installed_app().exists(), "no app installed when refusing");

    let r = k.install(&["--uninstall"]);
    assert!(!r.ok, "uninstall should report the foreign file: {}", r.out);
    assert!(k.cli().exists(), "uninstall removed a foreign file");
}

#[test]
fn a_foreign_symlink_or_app_stops_the_install() {
    let k = Kit::new();
    std::fs::create_dir_all(k.cli().parent().unwrap()).unwrap();
    std::os::unix::fs::symlink("/bin/echo", k.cli()).unwrap();
    let r = k.install(&[]);
    assert!(!r.ok, "install replaced a foreign symlink: {}", r.out);
    assert_eq!(std::fs::read_link(k.cli()).unwrap(), std::path::Path::new("/bin/echo"));
    std::fs::remove_file(k.cli()).unwrap();

    let app = k.installed_app();
    std::fs::create_dir_all(app.join("Contents")).unwrap();
    std::fs::write(app.join("Contents/readme"), "not leon").unwrap();
    let r = k.install(&[]);
    assert!(!r.ok, "install replaced a foreign Leon.app: {}", r.out);
    assert!(r.out.contains(&*app.display().to_string()), "names the foreign app: {}", r.out);
    assert_eq!(std::fs::read_to_string(app.join("Contents/readme")).unwrap(), "not leon");
    assert!(k.cli().symlink_metadata().is_err(), "no CLI link when refusing");
    let r = k.install(&["--uninstall"]);
    assert!(!r.ok && app.join("Contents/readme").exists(), "uninstall left the foreign app: {}", r.out);
}
