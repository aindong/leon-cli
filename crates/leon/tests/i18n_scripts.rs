//! The translation scripts (.agent/plans/2026-11-18-i18n-setup.md), on a throwaway repo with one
//! good and one broken translation: scripts/i18n-check.sh passes the good one and names each fault
//! of the broken one, and scripts/i18n-status.sh tells ok, stale and missing apart.

mod help_walk;

use std::path::Path;
use std::process::{Command, Output};

const SOURCE: &str = "# Install\n\nRun this:\n\n```sh\nleon new demo\n```\n\n![shot](images/a.png)\n";

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

fn write(dir: &Path, path: &str, text: &str) {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// A repo with the scripts, an English README and docs/install.md, committed; returns the commit.
fn repo(dir: &Path) -> String {
    git(dir, &["init", "-q"]);
    for s in ["i18n-check.sh", "i18n-status.sh"] {
        write(
            dir,
            &format!("scripts/{s}"),
            &std::fs::read_to_string(help_walk::repo().join("scripts").join(s)).unwrap(),
        );
    }
    write(dir, "README.md", "# leon\n");
    write(dir, "docs/install.md", SOURCE);
    write(dir, "docs/images/a.png", "png");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "english"]);
    git(dir, &["rev-parse", "--short=12", "HEAD"])
}

fn translation(commit: &str, code: &str, image: &str) -> String {
    format!(
        "<!-- i18n: source=docs/install.md commit={commit} -->\n\
         > これは翻訳です。英語の[ページ](../../install.md)が基準で、より新しい場合があります。\n\n\
         # インストール\n\n次を実行します:\n\n```sh\n{code}\n```\n\n![画面]({image})\n"
    )
}

fn run(dir: &Path, script: &str, args: &[&str]) -> Output {
    Command::new("bash").arg(dir.join("scripts").join(script)).args(args).output().unwrap()
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn the_check_passes_a_good_translation_and_names_each_fault_of_a_broken_one() {
    let d = tempfile::tempdir().unwrap();
    let commit = repo(d.path());
    write(d.path(), "docs/i18n/ja/install.md", &translation(&commit, "leon new demo", "../../images/a.png"));
    write(d.path(), "docs/i18n/README.md", "# Translations\n\n[日本語](ja/install.md)\n");
    let good = run(d.path(), "i18n-check.sh", &[]);
    assert!(good.status.success(), "{}", text(&good));
    assert!(text(&good).contains("2 files ok"), "{}", text(&good));

    // a translated command and a broken image link
    write(d.path(), "docs/i18n/ko/install.md", &translation(&commit, "leon new 데모", "../images/a.png"));
    let o = run(d.path(), "i18n-check.sh", &[]);
    let out = text(&o);
    assert_eq!(o.status.code(), Some(1), "{out}");
    assert!(out.contains("docs/i18n/ko/install.md: its code blocks differ"), "{out}");
    assert!(out.contains("docs/i18n/ko/install.md: links to ../images/a.png"), "{out}");
    assert!(!out.contains("docs/i18n/ja/"), "the good one has no fault: {out}");
    std::fs::remove_dir_all(d.path().join("docs/i18n/ko")).unwrap();

    // no header; a source that doesn't exist; a commit the clone doesn't have; a copied image
    let faults = [
        ("docs/i18n/es/install.md", "# Instalar\n".to_string(), "line 1 must be"),
        (
            "docs/i18n/es/install.md",
            translation(&commit, "leon new demo", "../../images/a.png").replace("docs/install.md", "docs/gone.md"),
            "its source docs/gone.md doesn't exist",
        ),
        (
            "docs/i18n/es/install.md",
            translation("0123456789ab", "leon new demo", "../../images/a.png"),
            "commit 0123456789ab has no docs/install.md",
        ),
        (
            "docs/i18n/es/install.md",
            translation(&commit, "leon new demo", "../../images/a.png")
                .replace("source=docs/install.md", "source=docs/i18n/ja/install.md"),
            "its source must be the English page",
        ),
        (
            "docs/i18n/es/setup.md",
            translation(&commit, "leon new demo", "../../images/a.png"),
            "is named after install.md",
        ),
        ("docs/i18n/es/a.png", "png".to_string(), "only Markdown lives in docs/i18n/"),
    ];
    for (path, body, said) in faults {
        write(d.path(), path, &body);
        let o = run(d.path(), "i18n-check.sh", &[]);
        assert_eq!(o.status.code(), Some(1), "{path}: {}", text(&o));
        assert!(text(&o).contains(said), "{path}: wants {said:?} in {}", text(&o));
        std::fs::remove_dir_all(d.path().join("docs/i18n/es")).unwrap();
    }
}

#[test]
fn the_status_tells_ok_stale_and_missing_apart() {
    let d = tempfile::tempdir().unwrap();
    let commit = repo(d.path());
    let none = run(d.path(), "i18n-status.sh", &[]);
    assert!(text(&none).contains("no translations yet"), "{}", text(&none));

    write(d.path(), "docs/i18n/ja/install.md", &translation(&commit, "leon new demo", "../../images/a.png"));
    let o = run(d.path(), "i18n-status.sh", &[]);
    assert!(o.status.success(), "{}", text(&o));
    let lines: Vec<String> = text(&o).lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    assert!(lines.contains(&"ja install.md ok".to_string()), "{lines:?}");
    assert!(lines.contains(&"ja README.md missing".to_string()), "{lines:?}");
    assert_eq!(lines.len(), 8, "the eight translated pages: {lines:?}");

    write(d.path(), "docs/install.md", &format!("{SOURCE}\nMore.\n"));
    git(d.path(), &["commit", "-qam", "more english"]);
    let o = run(d.path(), "i18n-status.sh", &["--markdown"]);
    let out = text(&o);
    assert!(out.starts_with("| Page | ja |\n|---|---|\n"), "{out}");
    assert!(out.contains("| install.md | stale (1) |"), "{out}");
    assert!(out.contains("| README.md | missing |"), "{out}");
}
