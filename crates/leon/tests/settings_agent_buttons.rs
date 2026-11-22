//! Settings → Agents: a row per agent with its state in plain words and the buttons that run the
//! real `leon skill setup|remove --agent X`. The window runs a wrapper that sets HOME,
//! CLAUDE_CONFIG_DIR and CODEX_HOME to a temp dir (and a PATH with no agent on it) before it
//! execs this build's leon, so no test touches the real ~/.claude, ~/.codex or ~/.agents.

// macOS only: the Settings window is in the app, which builds only there
#![cfg(all(feature = "app", target_os = "macos"))]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{px, size, Modifiers, TestAppContext, VisualTestContext, WindowHandle};
use leon_app::agent_skill::{AgentRow, Verb};
use leon_app::settings_window::SettingsWindow;

struct Kit {
    home: tempfile::TempDir,
    leon: PathBuf,
    _server: tempfile::TempDir,
    app: Rc<leon_app::embedded::Embedded>,
    sw: WindowHandle<SettingsWindow>,
}

impl Kit {
    fn claude(&self) -> PathBuf {
        self.home.path().join(".claude")
    }
    fn codex(&self) -> PathBuf {
        self.home.path().join(".codex")
    }
    fn skill(&self) -> PathBuf {
        self.claude().join("skills/leon/SKILL.md")
    }
}

/// A Settings window on a private server whose Agents runs leon in a temp home. `prepare` sets
/// that home up before the first status is read.
fn kit(cx: &mut TestAppContext, prepare: impl FnOnce(&Path)) -> (Kit, VisualTestContext) {
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale"); // never this machine's
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
    let server = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let app = Rc::new(leon_app::embedded::start_with(server.path().join("s.sock"), None, vec![]).unwrap());
    let remote = Rc::new(leon_app::remote::Remote::connect(app.socket()).unwrap());
    let config = server.path().join("init.lua");
    let sw = cx.update(|cx| SettingsWindow::open(config, remote, cx)).unwrap();
    let mut vcx = VisualTestContext::from_window(sw.into(), cx);
    vcx.simulate_resize(size(px(900.), px(2400.)));
    let k = Kit { home, leon, _server: server, app, sw };
    k.sw.update(&mut vcx, |sw, _, cx| sw.read_agents(k.leon.clone(), cx)).unwrap();
    assert!(until(&mut vcx, |cx| rows(cx, &k).is_some()), "the status came");
    click(&mut vcx, "section-agents");
    (k, vcx)
}

fn until(cx: &mut VisualTestContext, mut ok: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(20);
    while !ok(cx) && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    ok(cx)
}

fn rows(cx: &mut VisualTestContext, k: &Kit) -> Option<Vec<AgentRow>> {
    k.sw.read_with(cx, |sw, _| sw.agents()).unwrap().and_then(Result::ok)
}

fn row(cx: &mut VisualTestContext, k: &Kit, agent: &str) -> AgentRow {
    rows(cx, k).unwrap().into_iter().find(|r| r.agent == agent).unwrap()
}

fn drawn(cx: &mut VisualTestContext, sel: &'static str) -> bool {
    cx.run_until_parked();
    cx.debug_bounds(sel).is_some()
}

fn click(cx: &mut VisualTestContext, sel: &'static str) {
    cx.run_until_parked();
    let b = cx.debug_bounds(sel).unwrap_or_else(|| panic!("nothing drawn as {sel}"));
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
}

/// Click `sel` and wait until `agent`'s command ran and its row was read again with `status`.
fn press(cx: &mut VisualTestContext, k: &Kit, sel: &'static str, agent: &str, status: &str) -> AgentRow {
    click(cx, sel);
    let done = until(cx, |cx| {
        k.sw.read_with(cx, |sw, _| sw.agent_busy(agent)).unwrap().is_none() && row(cx, k, agent).status == status
    });
    let said = k.sw.read_with(cx, |sw, _| sw.agent_said(agent)).unwrap();
    assert!(done, "{sel}: {agent} never became {status}: {:?}, said {said:?}", row(cx, k, agent));
    row(cx, k, agent)
}

#[gpui::test]
fn install_update_and_remove_run_leon_skill_in_the_temp_home(cx: &mut TestAppContext) {
    let (k, mut cx) = kit(cx, |h| std::fs::create_dir(h.join(".claude")).unwrap());
    let cx = &mut cx;
    let first = row(cx, &k, "claude");
    assert_eq!((first.words(), first.verbs(), first.found), ("Not installed".into(), vec![Verb::Install], true));
    assert!(first.writes().contains("settings.json"), "it says what setup writes: {}", first.writes());
    let codex = row(cx, &k, "codex");
    assert!(!codex.found && codex.verbs().is_empty(), "Codex isn't here: no Install, {codex:?}");
    assert_eq!(codex.setup.as_deref(), Some("leon skill setup --agent codex"), "the command instead");

    let r = press(cx, &k, "agent-claude-install", "claude", "current");
    assert_eq!((r.words(), r.verbs()), ("Up to date".into(), vec![Verb::Remove]));
    assert!(k.skill().is_file() && k.claude().join("settings.json").is_file(), "written in the temp home");
    assert!(!k.codex().exists(), "nothing for the agent that isn't here");

    // an older skill file: Update brings it back
    let stub = std::fs::read_to_string(k.skill()).unwrap();
    std::fs::write(k.skill(), format!("{stub}an older line\n")).unwrap();
    k.sw.update(cx, |sw, _, cx| sw.read_agents(k.leon.clone(), cx)).unwrap();
    assert!(until(cx, |cx| rows(cx, &k).is_some_and(|r| r[0].status == "outdated")));
    let r = row(cx, &k, "claude");
    assert_eq!(
        (r.words(), r.verbs()),
        ("Update available: a newer skill file".into(), vec![Verb::Update, Verb::Remove])
    );
    press(cx, &k, "agent-claude-update", "claude", "current");
    assert_eq!(std::fs::read_to_string(k.skill()).unwrap(), stub);

    // the hooks of an older leon: one event short
    let settings = k.claude().join("settings.json");
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    v["hooks"].as_object_mut().unwrap().remove("Stop");
    std::fs::write(&settings, v.to_string()).unwrap();
    k.sw.update(cx, |sw, _, cx| sw.read_agents(k.leon.clone(), cx)).unwrap();
    assert!(until(cx, |cx| rows(cx, &k).is_some_and(|r| r[0].status == "outdated")));
    assert_eq!(row(cx, &k, "claude").words(), "Update available: new status hooks");
    press(cx, &k, "agent-claude-update", "claude", "current");

    let r = press(cx, &k, "agent-claude-remove", "claude", "missing");
    assert_eq!(r.verbs(), vec![Verb::Install]);
    assert!(!k.skill().exists(), "removed");
    k.app.stop();
}

#[gpui::test]
fn a_foreign_skill_is_never_written_and_says_the_command(cx: &mut TestAppContext) {
    let mine = "---\nname: leon\n---\nmy own notes\n";
    let (k, mut cx) = kit(cx, |h| {
        std::fs::create_dir_all(h.join(".claude/skills/leon")).unwrap();
        std::fs::write(h.join(".claude/skills/leon/SKILL.md"), mine).unwrap();
        std::fs::create_dir(h.join(".codex")).unwrap();
    });
    let cx = &mut cx;
    let r = row(cx, &k, "claude");
    assert_eq!((r.words(), r.verbs()), ("Changed by you: leon won't overwrite it".into(), vec![]));
    assert_eq!(r.setup.as_deref(), Some("leon skill setup --agent claude --force"), "the explicit command");
    for b in ["install", "update", "remove"] {
        assert!(!drawn(cx, format!("agent-claude-{b}").leak()), "no {b} button on a foreign skill");
    }
    // every button the section has, pressed: Codex's Install
    press(cx, &k, "agent-codex-install", "codex", "current");
    assert_eq!(std::fs::read_to_string(k.skill()).unwrap(), mine, "the foreign SKILL.md is untouched");
    assert!(std::fs::read_dir(k.skill().parent().unwrap()).unwrap().count() == 1, "and no backup made");
    k.app.stop();
}

#[gpui::test]
fn a_failed_setup_is_shown_in_the_row(cx: &mut TestAppContext) {
    // a skills folder setup may not write in: it can't make the skill's
    let (k, mut cx) = kit(cx, |h| {
        std::fs::create_dir_all(h.join(".claude/skills")).unwrap();
        std::fs::set_permissions(h.join(".claude/skills"), std::fs::Permissions::from_mode(0o500)).unwrap();
    });
    let cx = &mut cx;
    click(cx, "agent-claude-install");
    let said = |cx: &mut VisualTestContext| k.sw.read_with(cx, |sw, _| sw.agent_said("claude")).unwrap();
    assert!(until(cx, |cx| said(cx).is_some()), "the command ended");
    let shown = drawn(cx, "agent-claude-said");
    let got = said(cx).unwrap();
    k.app.stop();
    assert!(got.as_ref().is_err_and(|e| e.contains("skills")), "the error: {got:?}");
    assert!(shown, "drawn in the row");
}
