//! The command palette's rows and its fuzzy filter.

/// The app's own actions the palette offers, as (action name, title).
pub const BUILTINS: &[(&str, &str)] = &[
    ("session.new", "New Session"),
    ("session.switch", "Switch Session or Tab…"),
    ("client.tab.new", "New Tab"),
    ("pane.split.right", "Split Right"),
    ("pane.split.down", "Split Below"),
    ("pane.close", "Close Pane"),
    ("pane.zoom", "Zoom Pane"),
    ("pane.focus.left", "Focus Left"),
    ("pane.focus.right", "Focus Right"),
    ("pane.focus.up", "Focus Up"),
    ("pane.focus.down", "Focus Down"),
    ("client.tab.next", "Next Tab"),
    ("client.tab.previous", "Previous Tab"),
    ("client.sidebar.toggle", "Toggle Sidebar"),
    ("client.tab_style.toggle", "Toggle Tab Style"),
    ("client.pane_header.reveal", "Show or Hide Pane Headers"),
    ("client.font.increase", "Increase Font Size"),
    ("client.font.decrease", "Decrease Font Size"),
    ("client.font.reset", "Reset Font Size"),
    ("client.config.reload", "Reload Config"),
    ("pane.paste", "Paste"),
    ("pane.copy", "Copy"),
    ("pane.select_all", "Select All"),
    ("client.find.open", "Find"),
    ("client.find.next", "Find Next"),
    ("client.find.previous", "Find Previous"),
    ("client.scroll.page_up", "Scroll Up a Page"),
    ("client.scroll.page_down", "Scroll Down a Page"),
    ("client.scroll.top", "Scroll to the Top"),
    ("client.scroll.bottom", "Scroll to the Bottom"),
    ("client.host.add", "Add Host…"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub title: String,
    pub keywords: Vec<String>,
    /// The keys that run it (`⌘D`).
    pub shortcut: Option<String>,
    /// What the action runs with (`session.new` on a host: `{"host": "devbox"}`).
    pub args: Option<serde_json::Value>,
}

/// The indexes of the items matching `query`, best first; an empty query keeps every item in order.
/// A query character must appear in order in the title, the name or a keyword (a space matches
/// any word break); runs and word starts score higher.
pub fn filter(items: &[Item], query: &str) -> Vec<usize> {
    let q: Vec<char> = query.trim().to_lowercase().chars().collect();
    let mut scored: Vec<(u32, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| {
            let fields = std::iter::once(&it.title).chain([&it.name]).chain(&it.keywords);
            fields.filter_map(|f| score(&q, f)).max().map(|s| (s, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

fn is_break(c: char) -> bool {
    matches!(c, ' ' | '.' | '_' | '-' | '/')
}

/// shortcut: greedy leftmost matching, not the best alignment; enough for a few hundred short titles.
fn score(q: &[char], hay: &str) -> Option<u32> {
    let h: Vec<char> = hay.to_lowercase().chars().collect();
    let (mut at, mut total, mut last) = (0, 0, None);
    for &c in q {
        let i = (at..h.len()).find(|&i| if c == ' ' { is_break(h[i]) } else { h[i] == c })?;
        let word_start = i == 0 || is_break(h[i - 1]);
        total += 1 + if last == Some(i.wrapping_sub(1)) { 5 } else { 0 } + if word_start { 8 } else { 0 };
        (at, last) = (i + 1, Some(i));
    }
    Some(total)
}
