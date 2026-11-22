//! The `tailscale cert` command leon tells people to run (the server's `web_error`, Settings'
//! HTTPS hint) names a `tailscale` they have: the one on PATH, else the Mac app's binary, which the
//! standalone Tailscale app doesn't put on PATH. The files and the name are filled in.
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use leon_hosts::tailscale::{cert_command, cert_program};

fn exe(p: &Path) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_program_is_the_one_on_path_else_the_apps() {
    let d = tempfile::tempdir().unwrap();
    let app = d.path().join("Tailscale.app/Contents/MacOS/Tailscale");
    exe(&app);
    let bundles = [app.as_path()];
    let empty = d.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    assert_eq!(cert_program(Some(empty.as_os_str()), &bundles), format!("'{}'", app.display()), "the app's");
    let bin = d.path().join("bin");
    exe(&bin.join("tailscale"));
    assert_eq!(cert_program(Some(bin.as_os_str()), &bundles), "tailscale", "on PATH");
    assert_eq!(cert_program(Some(OsStr::new("")), &[]), "tailscale", "none found: the plain name");
}

#[test]
fn the_command_fills_in_the_files_and_the_name() {
    let c = cert_command(
        "tailscale",
        Path::new("/x/my certs/web.crt"),
        Path::new("/x/web.key"),
        "devbox.example-tailnet.ts.net",
    );
    assert!(
        c.contains(" cert --cert-file '/x/my certs/web.crt' --key-file '/x/web.key' devbox.example-tailnet.ts.net"),
        "{c}"
    );
    if cfg!(target_os = "linux") {
        assert!(c.starts_with("sudo ") && c.contains("chown"), "{c}");
    }
}
