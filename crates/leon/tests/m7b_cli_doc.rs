//! M7b criterion 6 (.agent/plans/2026-11-14-m7b-ship.md): docs/cli.md can't drift from `--help`.
//! Every command has a "`leon PATH …`" heading, every long flag of a command appears in its own
//! section (global flags anywhere), and every heading names a real command.

mod help_walk;

/// The command words of a heading like "### `leon move [BLOCK] DIRECTION ANCHOR`".
fn heading_path(line: &str) -> Option<Vec<String>> {
    let rest = line.trim_start_matches('#').trim().strip_prefix("`leon")?;
    let inner = rest.split('`').next()?;
    let words = inner
        .split_whitespace()
        .take_while(|w| w.chars().all(|c| c.is_ascii_lowercase() || c == '-') && !w.starts_with('-'));
    Some(words.map(str::to_string).collect())
}

/// `flag` appears as a whole word: `--for` isn't found inside `--force`.
fn mentions(text: &str, flag: &str) -> bool {
    text.match_indices(flag)
        .any(|(at, _)| !text[at + flag.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-'))
}

#[test]
fn cli_md_covers_every_command_and_flag() {
    let doc = std::fs::read_to_string(help_walk::repo().join("docs/cli.md")).expect("docs/cli.md exists");
    let lines: Vec<&str> = doc.lines().collect();
    let all = help_walk::all();
    let globals = all.iter().find(|h| h.path.is_empty()).unwrap().long_flags();
    let mut problems = vec![];
    for g in &globals {
        if !mentions(&doc, g) {
            problems.push(format!("global flag {g} is not in the doc"));
        }
    }
    let headings: Vec<(usize, Vec<String>)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with('#'))
        .filter_map(|(i, l)| heading_path(l).map(|p| (i, p)))
        .collect();
    for h in all.iter().filter(|h| !h.path.is_empty()) {
        let Some(&(at, _)) = headings.iter().find(|(_, p)| *p == h.path) else {
            problems.push(format!("no heading for `leon {}`", h.path.join(" ")));
            continue;
        };
        let end =
            lines.iter().enumerate().skip(at + 1).find(|(_, l)| l.starts_with('#')).map_or(lines.len(), |(i, _)| i);
        let section = lines[at..end].join("\n");
        for f in h.long_flags().iter().filter(|f| !globals.contains(f)) {
            if !mentions(&section, f) {
                problems.push(format!("`leon {}`: {f} is not in its section", h.path.join(" ")));
            }
        }
    }
    for (_, p) in &headings {
        if !p.is_empty() && !all.iter().any(|h| h.path == *p) {
            problems.push(format!("heading names `leon {}`, which is not a command", p.join(" ")));
        }
    }
    assert!(problems.is_empty(), "docs/cli.md drifted from --help:\n{}", problems.join("\n"));
}
