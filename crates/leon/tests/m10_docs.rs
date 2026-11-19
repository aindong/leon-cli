//! M10 criterion 9 (.agent/plans/2026-11-14-m10-app-attaches.md): the docs say the app attaches
//! and programs outlive it, and nothing still says the opposite.

fn doc(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// `text` with line breaks and runs of spaces folded, so a phrase can't hide across a wrap.
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

#[test]
fn the_docs_say_programs_outlive_the_app() {
    let readme = flat(&doc("README.md"));
    let bullet = readme.split("**persistent layout.**").nth(1).expect("README keeps its Persistent layout bullet");
    let bullet = bullet.split("- **").next().unwrap();
    assert!(bullet.contains("keep running when the app quits"), "the bullet: {bullet}");

    let arch = flat(
        &["docs/architecture.md", "docs/architecture/protocol-and-clients.md", "docs/architecture/state-and-hosts.md"]
            .map(doc)
            .concat(),
    );
    assert!(arch.contains("**the app attaches.**"), "architecture.md has an 'The app attaches' decision");
    assert!(arch.contains("block.watch") && arch.contains("dirty"), "and describes the screen transport");
    let cli = flat(&doc("docs/cli.md"));
    let app = cli.split("### `leon app`").nth(1).unwrap().split("## ").next().unwrap();
    assert!(app.contains("attach"), "cli.md's leon app section says it attaches: {app}");
    let perf = flat(&doc("docs/performance.md"));
    assert!(
        perf.contains("keystroke to screen") && perf.contains("seq 1 200000"),
        "performance.md has the M10 numbers"
    );

    let brand = [
        "brand/README.md",
        "brand/marketing/index.html",
        "brand/marketing/sections/questions.html",
        "brand/marketing/copy.md",
    ]
    .map(|p| (p, flat(&doc(p))));
    let docs = [("README.md", &readme), ("docs/architecture.md", &arch), ("docs/cli.md", &cli)];
    for (path, text) in docs.into_iter().chain(brand.iter().map(|(p, t)| (*p, t))) {
        for stale in [
            "quitting the app ends them",
            "closing the window ends every program",
            "the app exits 1",
            "the app owns its server",
            "runs the server inside the app process",
            "quitting the app ends",
            "do not claim running jobs survive quitting the app",
        ] {
            assert!(!text.contains(stale), "{path} still says {stale:?}");
        }
    }
}
