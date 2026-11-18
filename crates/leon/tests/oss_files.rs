//! Open-source basics (.agent/plans/2026-11-18-i18n-setup.md): the README links every language and
//! the community files. Those files and docs/i18n/ start no CI run when changed alone, so no cargo
//! test reads them (pub_doc_links only checks that the README's links to them resolve).

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
