//! Leon.app, started with an agent's skill out of date, says so once per app version: "Agent
//! skill out of date" with Update, which runs the real `leon skill setup` for it. Never for an
//! agent whose skill is missing or someone else's. The app's `leon` here is a wrapper that sets
//! HOME, CLAUDE_CONFIG_DIR and CODEX_HOME to a temp dir, so the real ones are never touched.

// macOS only: Leon.app and its notices exist only there
#![cfg(all(feature = "app", target_os = "macos"))]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{px, size, Modifiers, TestAppContext, VisualTestContext};
use leon_app::view::{self, Workspace};

/// A temp home with `prepare` done, and the leon that runs in it.
fn home(prepare: impl FnOnce(&Path)) -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().unwrap();
    prepare(home.path());
    let leon = home.path().join("leon-in-home");
    let h = home.path().display();
    let script = format!(
        "#!/bin/sh\nexport HOME='{h}' CLAUDE_CONFIG_DIR='{h}/.claude' CODEX_HOME='{h}/.codex' PATH=/usr/bin:/bin\nexec '{}' \"$@\"\n",
        env!("CARGO_BIN_EXE_leon")
    );
    std::fs::write(&leon, script).unwrap();
    std::fs::set_permissions(&leon, std::fs::Permissions::from_mode(0o755)).unwrap();
    (home, leon)
}

fn leon(leon: &Path, args: &[&str]) {
    let out = std::process::Command::new(leon).args(args).output().unwrap();
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Claude Code set up, then its skill file made an older leon's.
fn outdated(h: &Path, bin: &Path) {
    leon(bin, &["skill", "setup", "--agent", "claude"]);
    let skill = h.join(".claude/skills/leon/SKILL.md");
    let stub = std::fs::read_to_string(&skill).unwrap();
    std::fs::write(&skill, format!("{stub}an older line\n")).unwrap();
}

/// A start of the app: a workspace with `store` as its updates.json, checking the skill with
/// `bin`; the notice it shows once the check is done (None: none).
fn start(cx: &mut TestAppContext, store: &Path, bin: &Path) -> Option<String> {
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale"); // never this machine's
    let server = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let app = Rc::new(leon_app::embedded::start_with(server.path().join("s.sock"), None, vec![]).unwrap());
    cx.update(view::init);
    let socket = app.socket().to_path_buf();
    let (ws, vcx) = cx.add_window_view(move |window, cx| {
        Workspace::new(Rc::new(leon_app::remote::Remote::connect(&socket).unwrap()), window, cx)
    });
    vcx.simulate_resize(size(px(1400.), px(900.)));
    let (store, bin) = (store.to_path_buf(), bin.to_path_buf());
    ws.update(vcx, |ws, cx| {
        ws.enable_updates(Some(store), Arc::new(|| Err("no install in a test".into())));
        ws.check_agent_skill(bin, cx);
    });
    let checked = until(vcx, |cx| ws.read_with(cx, |ws, _| ws.agent_skill_checked()));
    assert!(checked, "the check ran");
    let text = ws.read_with(vcx, |ws, _| ws.notice_text());
    app.stop();
    text
}

fn until(cx: &mut VisualTestContext, mut ok: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(20);
    while !ok(cx) && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    ok(cx)
}

#[gpui::test]
fn once_per_version_and_update_brings_it_up_to_date(cx: &mut TestAppContext) {
    let (h, bin) = home(|h| std::fs::create_dir(h.join(".claude")).unwrap());
    outdated(h.path(), &bin);
    let state = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let store = state.path().join("updates.json"); // saved only in a private dir, as the state dir is
    let first = start(cx, &store, &bin);
    let second = start(cx, &store, &bin);
    // a store an older version wrote: this version hasn't shown it
    let mut saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    saved["skill_notice"] = "0.0.1".into();
    std::fs::write(&store, saved.to_string()).unwrap();
    assert!(first.as_deref().is_some_and(|t| t.starts_with("Agent skill out of date")), "{first:?}");
    assert!(first.unwrap().contains("Claude Code"), "names the agent");
    assert_eq!(second, None, "the same version shows it once");

    // the third start: shown again, and its Update runs setup
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale");
    let server = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let app = Rc::new(leon_app::embedded::start_with(server.path().join("s.sock"), None, vec![]).unwrap());
    cx.update(view::init);
    let socket = app.socket().to_path_buf();
    let (ws, vcx) = cx.add_window_view(move |window, cx| {
        Workspace::new(Rc::new(leon_app::remote::Remote::connect(&socket).unwrap()), window, cx)
    });
    vcx.simulate_resize(size(px(1400.), px(900.)));
    let (s, b) = (store.clone(), bin.clone());
    ws.update(vcx, |ws, cx| {
        ws.enable_updates(Some(s), Arc::new(|| Err("no install in a test".into())));
        ws.check_agent_skill(b, cx);
    });
    let text = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.notice_text());
    assert!(until(vcx, |cx| text(cx).is_some_and(|t| t.starts_with("Agent skill out of date"))), "shown again");
    vcx.run_until_parked();
    let b = vcx.debug_bounds("skill-update").expect("the notice's Update");
    vcx.simulate_click(b.center(), Modifiers::none());
    let updated = until(vcx, |cx| text(cx).is_some_and(|t| t.starts_with("Agent skill updated")));
    let after = text(vcx);
    app.stop();
    assert!(updated, "{after:?}");
    let out = std::process::Command::new(&bin).args(["--json", "skill", "status", "--agent", "claude"]).output();
    let v: serde_json::Value = serde_json::from_slice(&out.unwrap().stdout).unwrap();
    assert_eq!(v[0]["status"], "current", "Update ran leon skill setup: {v}");
}

#[gpui::test]
fn missing_or_foreign_skills_show_nothing(cx: &mut TestAppContext) {
    let (h, bin) = home(|h| {
        std::fs::create_dir(h.join(".codex")).unwrap(); // here, never set up
        std::fs::create_dir_all(h.join(".claude/skills/leon")).unwrap();
        std::fs::write(h.join(".claude/skills/leon/SKILL.md"), "---\nname: leon\n---\nmine\n").unwrap();
    });
    let shown = start(cx, &h.path().join("updates.json"), &bin);
    assert_eq!(shown, None);
}
