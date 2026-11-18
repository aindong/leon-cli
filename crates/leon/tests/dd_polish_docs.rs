//! Daily-driver polish (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 9): the docs carry
//! what changed. The words are the deliverable here, so the test reads them.

mod help_walk;

fn read(name: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn the_docs_say_what_changed() {
    // its detail shipped in 0.1.0, in docs/changelog-0.1.0.md (file-size limit)
    let detail = read("docs/changelog-0.1.0.md");
    for topic in ["run --wait", "--output", "wait --text", "skill setup", "PATH", "claude -p", "not sent", "drawer"] {
        assert!(detail.contains(topic), "the changelog's 0.1.0 detail should mention {topic}");
    }

    let cli = read("docs/cli.md");
    let section = |h: &str| cli.split(h).nth(1).unwrap().split("\n### ").next().unwrap().to_string();
    let run = section("### `leon run [FLAGS] -- CMD…`");
    assert!(run.contains("--output") && run.contains("not a terminal"), "docs/cli.md: run --output and when it prints");
    let wait = section("### `leon wait [BLOCK]`");
    assert!(wait.contains("--text"), "docs/cli.md: wait --text");

    let readme = read("README.md");
    let first = readme.split("## The first five commands").nth(1).unwrap();
    let block = help_walk::blocks(first, "sh").into_iter().next().unwrap();
    let wait_at = block.find("wait -b py --text").expect("the README waits for the prompt");
    assert!(wait_at < block.find(" send ").unwrap(), "it waits before it types");

    let guide = read("crates/leon/src/skill/guide.md");
    let recipe = guide.split("Drive an interactive program").nth(1).expect("the guide's interactive recipe");
    let recipe = help_walk::blocks(recipe, "sh").into_iter().next().unwrap();
    assert!(recipe.contains("leon wait") && recipe.contains("--text"), "the guide's recipe waits for the prompt");
}
