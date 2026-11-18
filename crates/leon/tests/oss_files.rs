//! Open-source basics (.agent/plans/2026-11-18-i18n-setup.md): the community files are there, the
//! README links them and every language, and no contact in them is an email address.

mod help_walk;

fn read(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_readme_switches_language_and_links_the_community_files() {
    let readme = read("README.md");
    let switcher = "English · [简体中文](docs/i18n/zh-CN/README.md) · [日本語](docs/i18n/ja/README.md) · \
                    [한국어](docs/i18n/ko/README.md) · [Español](docs/i18n/es/README.md) · \
                    [Português (Brasil)](docs/i18n/pt-BR/README.md)";
    let at = readme.find(switcher).expect("the README has the language switcher");
    assert!(at < readme.find("docs/images/banner.png").unwrap(), "near the top, above the banner");
    let section = readme.split("\n## Contributing\n").nth(1).expect("a Contributing section");
    let section = section.split("\n## ").next().unwrap();
    for link in ["](CONTRIBUTING.md)", "](CODE_OF_CONDUCT.md)", "](SECURITY.md)", "](docs/i18n/README.md)"] {
        assert!(section.contains(link), "Contributing links {link}");
    }
    assert!(readme.contains("\n## License\n"), "and a license section");
}

#[test]
fn the_community_files_are_there_and_name_no_email() {
    let contributing = read("CONTRIBUTING.md");
    for needle in ["## Translating", "docs/i18n/README.md", "`dev`", "LEON_RUNTIME_DIR", "scripts/demo-shots.sh"] {
        assert!(contributing.contains(needle), "CONTRIBUTING.md mentions {needle}");
    }
    assert!(read("CODE_OF_CONDUCT.md").contains("version 2.1"), "the Contributor Covenant 2.1");
    assert!(read("SECURITY.md").contains("Report a vulnerability"), "private vulnerability reporting");
    for t in ["bug_report.yml", "feature_request.yml", "translation.yml", "config.yml"] {
        read(&format!(".github/ISSUE_TEMPLATE/{t}"));
    }
    read(".github/pull_request_template.md");
    read("docs/i18n/README.md");
    let email = |text: &str| {
        text.split(|c: char| c.is_whitespace() || "<>()[]`'\"".contains(c))
            .any(|w| w.split_once('@').is_some_and(|(user, host)| !user.is_empty() && host.contains('.')))
    };
    for f in ["CONTRIBUTING.md", "CODE_OF_CONDUCT.md", "SECURITY.md", "docs/i18n/README.md"] {
        assert!(!email(&read(f)), "{f} names no email address");
    }
}
