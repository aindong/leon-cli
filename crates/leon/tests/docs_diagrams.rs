//! Diagrams criterion 1 (.agent/plans/2026-11-19-docs-visuals.md): the pages where a picture says
//! more than prose carry a Mermaid diagram, which GitHub and the website both draw.

mod help_walk;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The bodies of a Markdown file's ```mermaid blocks. A fence closes on a run of its own character
/// at least as long, with nothing after it, so a mermaid example inside a longer fence isn't one.
fn diagrams(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<(char, usize, Option<String>)> = None;
    for line in text.lines() {
        let t = line.trim_start();
        let c = t.chars().next().filter(|c| *c == '`' || *c == '~');
        let run = c.map_or(0, |c| t.chars().take_while(|x| *x == c).count());
        match &mut open {
            Some((fc, n, body)) if c == Some(*fc) && run >= *n && t[run..].trim().is_empty() => {
                out.extend(body.take());
                open = None;
            }
            Some((_, _, Some(body))) => body.push_str(&format!("{line}\n")),
            Some(_) => {}
            None if run >= 3 => {
                let info = t[run..].trim();
                open = Some((c.unwrap(), run, (info == "mermaid").then(String::new)));
            }
            None => {}
        }
    }
    out
}

/// The current login and host name, which no diagram may show.
fn private_names() -> Vec<String> {
    let host = std::process::Command::new("hostname").output().ok();
    let host = host.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase()).unwrap_or_default();
    let names =
        [std::env::var("USER").unwrap_or_default().to_lowercase(), host.split('.').next().unwrap_or("").to_string()];
    names.into_iter().filter(|n| n.len() >= 3).collect()
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
            for name in private_names() {
                assert!(!d.to_lowercase().contains(&name), "{page}: a diagram shows {name:?}");
            }
        }
    }
}

#[test]
fn diagrams_are_parsed_as_code_blocks() {
    let found = diagrams("a\n```mermaid\nflowchart LR\n  A --> B\n```\n```sh\nls\n```\n");
    assert_eq!(found, vec!["flowchart LR\n  A --> B\n".to_string()]);
    // an example inside a longer fence is not a diagram; a tilde fence is one
    assert!(diagrams("````md\n```mermaid\ngraph TD\n```\n````\n").is_empty());
    assert_eq!(diagrams("~~~mermaid\nflowchart TD\n~~~\n"), vec!["flowchart TD\n".to_string()]);
}
