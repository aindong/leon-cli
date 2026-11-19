//! Release kit criteria 4–5 (.agent/plans/2026-11-19-release-kit.md): the clips' cursor shader is
//! leon's own, under leon's license, and compiles as the app compiles it; scripts/demo-clips.sh
//! records from the neutral demo with keys only for the app it started; and the release steps
//! say to record the clips, put them in the changelog and the media release.

mod support;

fn read(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_clips_shader_is_leon_s_own() {
    let shader = read("scripts/demo-clips/trail.glsl");
    assert!(shader.contains("Written for leon and licensed\n// with it under the MIT license"), "{shader}");
    assert!(read("LICENSE").starts_with("MIT License"));
    assert!(read("scripts/demo-clips.sh").contains("scripts/demo-clips/trail.glsl"));
}

// macOS only: `leon config check` translates a cursor shader on macOS only.
#[cfg(target_os = "macos")]
#[test]
fn the_clips_shader_compiles() {
    let env = support::Env::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shader = root.join("scripts/demo-clips/trail.glsl").canonicalize().unwrap();
    let init = env.dir.path().join("init.lua");
    std::fs::write(&init, format!("leon.set(\"cursor_shader\", {:?})\n", shader.to_str().unwrap())).unwrap();
    let o = env.cmd().env("LEON_CONFIG", &init).args(["config", "check"]).output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "{text}");
    assert!(text.contains(": ok"), "{text}");
}

#[test]
fn the_clips_come_from_the_neutral_demo_in_the_vm() {
    let guest = read("scripts/demo-clips/guest.sh");
    for scene in ["agents)", "link-card)", "tabs)", "cursor-shader)"] {
        assert!(guest.matches(&format!("    {scene}")).count() == 2, "{scene} is staged and played");
    }
    for must in [
        "PROMPT='\\$ '",      // the neutral prompt
        "d=$HOME/leon-clips", // leon dirs of its own, never the take's
        "export LEON_RUNTIME_DIR=$d/run",
        "k() { keypost \"$@\"; }", // keys and clicks posted inside the VM
    ] {
        assert!(guest.contains(must), "guest.sh: {must}");
    }
    let host = read("scripts/demo-clips.sh");
    // nothing on this Mac's screen: no keys, clicks or recording here, only in the VM over ssh
    for never in ["osascript", "System Events", "CGEvent", "deskshot"] {
        assert!(!host.contains(never), "demo-clips.sh uses {never} on the host");
    }
    assert!(host.contains("ssh_vm \"rm -f ~/clip.mov; screencapture"), "the recorder runs in the VM");
    assert!(host.contains("max=${3:-1500000}"), "a clip's GIF is at most 1.5 MB");
}

#[test]
fn the_release_steps_say_how_to_show_what_it_brings() {
    let docs = read("docs/releases.md");
    let cutting = docs.split("## Cutting a release").nth(1).unwrap().split("\n## ").next().unwrap();
    for step in ["scripts/demo-clips.sh", "docs/images/whats-new/X.Y.Z/", "gh release upload media", "New in X.Y"] {
        assert!(cutting.contains(step), "Cutting a release: {step}");
    }
    assert!(docs.contains("deploys [the website](#the-website)"), "the site deploys after the Release run");
    assert!(read("docs/updates.md").contains("Updated to leon"), "updates.md says what the app shows");
}
