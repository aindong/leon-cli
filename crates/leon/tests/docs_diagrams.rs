//! Diagrams criterion 1 (.agent/plans/2026-11-19-docs-visuals.md): the pages where a picture says
//! more than prose carry a Mermaid diagram, which GitHub and the website both draw.

mod help_walk;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The bodies of a Markdown file's ```mermaid blocks.
fn diagrams(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut body: Option<String> = None;
    for line in text.lines() {
        match &mut body {
            Some(b) if line.trim_start().starts_with("```") => out.push(std::mem::take(b)),
            Some(b) => b.push_str(&format!("{line}\n")),
            None => {}
        }
        if line.trim_start().starts_with("```") {
            body = if body.is_none() && line.trim() == "```mermaid" { Some(String::new()) } else { None };
        }
    }
    out
}

#[test]
fn the_pages_have_their_diagrams() {
    for (page, want) in [
        ("docs/architecture.md", 2),
        ("docs/agents.md", 2),
        ("docs/remote-hosts.md", 1),
        ("docs/web.md", 1),
        ("docs/architecture/tailnet.md", 1),
        ("docs/architecture/scrollback.md", 1),
        ("docs/releases.md", 1),
        ("docs/updates.md", 1),
    ] {
        let found = diagrams(&doc(page));
        assert!(found.len() >= want, "{page} has {want} Mermaid diagram(s), found {}", found.len());
        for d in found {
            let kind = d.split_whitespace().next().unwrap_or("");
            assert!(["flowchart", "sequenceDiagram"].contains(&kind), "{page}: plain flowchart or sequence: {kind}");
            // a backtick in a label is Mermaid's Markdown string, which GitHub and the site draw differently
            assert!(!d.contains('`'), "{page}: no backtick in a diagram:\n{d}");
        }
    }
}

#[test]
fn diagrams_are_parsed_as_code_blocks() {
    let found = diagrams("a\n```mermaid\nflowchart LR\n  A --> B\n```\n```sh\nls\n```\n");
    assert_eq!(found, vec!["flowchart LR\n  A --> B\n".to_string()]);
}
