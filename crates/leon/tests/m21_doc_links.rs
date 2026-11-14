//! Every moved top-level section is reachable from the original short index.
use std::path::Path;

#[test]
fn split_docs_have_complete_indexes_and_no_dangling_local_links() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, count) in [("docs/spec", 9), ("docs/research/user-feedback", 6), ("docs/research/next-for-leon", 5)] {
        let index = root.join(format!("{name}.md"));
        let text = std::fs::read_to_string(&index).unwrap();
        assert!(text.len() < 5000, "short index: {name}");
        let parts: Vec<_> = std::fs::read_dir(root.join(name)).unwrap().map(|p| p.unwrap().path()).collect();
        assert_eq!(parts.len(), count, "one file per original top-level section");
        for part in parts {
            let rel = format!(
                "{}/{}",
                Path::new(name).file_name().unwrap().to_str().unwrap(),
                part.file_name().unwrap().to_str().unwrap()
            );
            assert!(text.contains(&format!("]({rel})")), "{name} must link {rel}");
            let content = std::fs::read_to_string(&part).unwrap();
            assert!(content.starts_with("## ") && content.len() <= 20_000, "{}", part.display());
            assert_eq!(content.lines().filter(|l| l.starts_with("## ")).count(), 1);
            check_links(&part, &content);
        }
        check_links(&index, &text);
    }
}
fn check_links(file: &Path, text: &str) {
    for suffix in text.split("](").skip(1) {
        let target = suffix.split(')').next().unwrap().split('#').next().unwrap();
        if target.is_empty() || target.contains("://") {
            continue;
        }
        assert!(file.parent().unwrap().join(target).exists(), "{}: missing {target}", file.display());
    }
}
