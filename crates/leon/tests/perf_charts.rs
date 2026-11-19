//! Charts criteria 4 and 5 (.agent/plans/2026-11-19-docs-visuals.md): each benchmark chart in
//! docs/images/perf/ is drawn from its data file in docs/perf/ by scripts/perf-charts.py, is current
//! with that data, and is shown on a performance page with alt text that says what it shows.

use std::process::Command;

mod help_walk;

const CHARTS: [&str; 8] =
    ["throughput", "keystroke", "key-to-pixel", "memory-soak", "memory-per-pane", "idle-cpu", "frame-build", "flood"];

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_charts_are_current_with_their_data() {
    let out = Command::new("python3")
        .arg(help_walk::repo().join("scripts/perf-charts.py"))
        .arg("--check")
        .output()
        .expect("python3 runs scripts/perf-charts.py");
    assert!(
        out.status.success(),
        "a chart is stale: run python3 scripts/perf-charts.py\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn each_chart_has_its_data_a_title_and_a_page() {
    let pages: String =
        ["docs/performance.md", "docs/performance-app.md", "docs/performance-cat.md", "docs/performance-memory.md"]
            .map(doc)
            .concat();
    for name in CHARTS {
        let csv = doc(&format!("docs/perf/{name}.csv"));
        assert!(csv.lines().count() > 1, "docs/perf/{name}.csv has rows");
        let svg = doc(&format!("docs/images/perf/{name}.svg"));
        for tag in ["<title", "<desc", "prefers-color-scheme: dark"] {
            assert!(svg.contains(tag), "{name}.svg has {tag}");
        }
        let link = format!("](images/perf/{name}.svg)");
        let at = pages.find(&link).unwrap_or_else(|| panic!("a performance page shows {name}.svg"));
        let alt = &pages[pages[..at].rfind("![").unwrap() + 2..at];
        assert!(alt.len() > 40, "{name}.svg's alt text says the takeaway: {alt:?}");
    }
    let first = doc("docs/performance.md");
    let headline = first.split("\n## ").next().unwrap();
    for name in ["throughput", "key-to-pixel", "memory-soak", "idle-cpu"] {
        assert!(headline.contains(&format!("images/perf/{name}.svg")), "performance.md opens with {name}.svg");
    }
}
