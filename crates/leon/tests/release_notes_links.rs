//! A release's notes link to the tagged commit on GitHub: CHANGELOG.md's links are relative to
//! the repo, which a GitHub Release page can't resolve, so `scripts/release-notes.sh` makes them
//! absolute; CHANGELOG.md itself stays relative.

mod release_kit;

use release_kit::{script, text};

#[test]
fn relative_links_point_at_the_tag_and_absolute_ones_stay() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("CHANGELOG.md");
    let body = "- one ([detail](docs/changelog-detail.md#one)), [roadmap](docs/roadmap.md)\n\
                - [up](#reading-api_version) and [kac](https://keepachangelog.com/en/1.1.0/), [me](mailto:a@b.c)\n\
                - an image ![icon](docs/images/icon.png)\n\n[milestones]: docs/changelog-milestones.md\n[up]: #reading-api_version\n\
                [kac]: https://keepachangelog.com/en/1.1.0/\n[me]: mailto:a@b.c\n";
    std::fs::write(&file, format!("# Changelog\n\n## [2.3.4] - 2030-01-01\n\n{body}\n## [2.3.3]\n\n- old\n")).unwrap();
    let o = script("release-notes.sh", &["2.3.4", file.to_str().unwrap()], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let (base, raw) =
        ("https://github.com/aindong/leon-cli/blob/v2.3.4", "https://github.com/aindong/leon-cli/raw/v2.3.4");
    let want = format!(
        "- one ([detail]({base}/docs/changelog-detail.md#one)), [roadmap]({base}/docs/roadmap.md)\n\
         - [up]({base}/CHANGELOG.md#reading-api_version) and [kac](https://keepachangelog.com/en/1.1.0/), [me](mailto:a@b.c)\n\
         - an image ![icon]({raw}/docs/images/icon.png)\n\n[milestones]: {base}/docs/changelog-milestones.md\n[up]: {base}/CHANGELOG.md#reading-api_version\n\
         [kac]: https://keepachangelog.com/en/1.1.0/\n[me]: mailto:a@b.c"
    );
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), want);
    assert!(std::fs::read_to_string(&file).unwrap().contains("](docs/roadmap.md)"), "the changelog stays relative");
}
