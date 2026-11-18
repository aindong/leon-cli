//! M13 criterion 11: the docs show a neutral demo, never the machine they were made on: no image
//! name, README text, image byte or (with macOS Vision, through scripts/ocr.swift) text read off
//! an image (every distinct frame of a GIF, with ffmpeg) holds the current user's login or this
//! machine's host or computer name.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The names that would give the machine away, lowercased: login, host name (and its first
/// label), and on macOS the local host name and computer name.
fn private_names() -> Vec<String> {
    let mut names = vec![std::env::var("USER").ok(), output("id", &["-un"]), output("hostname", &[])];
    names.push(output("hostname", &[]).and_then(|h| h.split('.').next().map(String::from)));
    names.push(output("scutil", &["--get", "LocalHostName"]));
    names.push(output("scutil", &["--get", "ComputerName"]));
    let mut names: Vec<String> =
        names.into_iter().flatten().map(|n| n.to_lowercase().replace('’', "'")).filter(|n| n.len() >= 3).collect();
    names.sort();
    names.dedup();
    names
}

/// Whether `name` is in `text` as a whole word (a login like `dev` inside `devbox` is no leak).
fn mentions(text: &str, name: &str) -> bool {
    let text = text.to_lowercase().replace('’', "'");
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    text.match_indices(name)
        .any(|(i, _)| !word(text[..i].chars().next_back()) && !word(text[i + name.len()..].chars().next()))
}

fn images() -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> =
        std::fs::read_dir(root().join("docs/images")).unwrap().map(|e| e.unwrap().path()).collect();
    all.sort();
    all
}

/// Every file under docs/i18n/ (none before the first translation lands).
fn translations() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out)
            } else {
                out.push(p)
            }
        }
    }
    let mut out = vec![];
    walk(&root().join("docs/i18n"), &mut out);
    out
}

#[test]
fn image_names_readme_and_image_bytes_name_no_user_or_machine() {
    let names = private_names();
    assert!(!names.is_empty(), "found the login and hostname to look for");
    // the README and the pages split out of it (file-size limit), and every translation
    let mut pages: Vec<(String, String)> =
        ["README.md", "docs/install.md", "docs/shortcuts.md", "docs/remote-hosts.md"]
            .map(|p| (p.to_string(), std::fs::read_to_string(root().join(p)).unwrap()))
            .into();
    pages.extend(translations().into_iter().map(|p| (p.display().to_string(), std::fs::read_to_string(&p).unwrap())));
    for name in &names {
        for (page, text) in &pages {
            assert!(!mentions(text, name), "{page} mentions {name:?}");
        }
        for img in images() {
            let file = img.file_name().unwrap().to_string_lossy().to_string();
            assert!(!mentions(&file, name), "{file} is named after {name:?}");
            let bytes = String::from_utf8_lossy(&std::fs::read(&img).unwrap()).to_string();
            assert!(!mentions(&bytes, name), "{file} holds {name:?} in its bytes (a text chunk?)");
        }
    }
}

#[test]
fn no_screenshot_shows_the_user_or_machine() {
    if output("swift", &["--version"]).is_none() || cfg!(not(target_os = "macos")) {
        eprintln!("no swift with Vision here: the OCR check is skipped");
        return;
    }
    let mut pngs: Vec<PathBuf> = images().into_iter().filter(|p| p.extension().is_some_and(|e| e == "png")).collect();
    // a recording's every distinct frame (mpdecimate drops repeats), when ffmpeg is here to split it
    let frames = tempfile::tempdir().unwrap();
    for gif in images().into_iter().filter(|p| p.extension().is_some_and(|e| e == "gif")) {
        let name = gif.file_stem().unwrap().to_string_lossy().to_string();
        let pattern = frames.path().join(format!("{name}-%04d.png"));
        let split = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-i"])
            .arg(&gif)
            .args(["-vf", "mpdecimate", "-fps_mode", "vfr"])
            .arg(&pattern)
            .status();
        if !split.is_ok_and(|s| s.success()) {
            eprintln!("no ffmpeg here: {name}.gif's frames are not OCR'd");
        }
    }
    let mut split: Vec<PathBuf> = std::fs::read_dir(frames.path()).unwrap().map(|e| e.unwrap().path()).collect();
    split.sort();
    pngs.extend(split);
    let mut cmd = Command::new("swift");
    cmd.arg(root().join("scripts/ocr.swift")).args(&pngs);
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "ocr failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    for name in private_names() {
        let hits: Vec<&str> =
            text.lines().filter(|l| mentions(l.split_once(": ").map_or(*l, |(_, t)| t), &name)).collect();
        assert!(hits.is_empty(), "screenshots show {name:?}:\n{}", hits.join("\n"));
    }
}

#[test]
fn claude_md_states_the_rule() {
    let rules = std::fs::read_to_string(root().join("CLAUDE.md")).unwrap();
    assert!(rules.contains("scripts/demo-shots.sh"), "CLAUDE.md says how screenshots are made");
    assert!(rules.contains("m13_privacy"), "and which test holds them to it");
}
