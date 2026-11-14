//! M7b criterion 4 (.agent/plans/2026-11-14-m7b-ship.md): every command, argument and option in
//! every `leon … --help` has a description on its line: words of its own, not just clap's
//! `[default: …]` / `[possible values: …]` notes.

mod help_walk;

#[test]
fn every_command_argument_and_option_is_described() {
    let mut missing = vec![];
    let all = help_walk::all();
    assert!(all.len() > 30, "the walk should reach every command, got {}", all.len());
    for h in &all {
        for e in &h.entries {
            if words(&e.desc).is_empty() {
                missing.push(format!("leon {}: {} `{}`", h.path.join(" "), e.section, e.spec));
            }
        }
    }
    assert!(missing.is_empty(), "no description:\n{}", missing.join("\n"));
}

/// The description without clap's bracketed notes.
fn words(desc: &str) -> String {
    let mut out = desc.to_string();
    for note in ["[default:", "[possible values:", "[env:", "[aliases:"] {
        while let Some(at) = out.find(note) {
            let end = out[at..].find(']').map_or(out.len(), |e| at + e + 1);
            out.replace_range(at..end, "");
        }
    }
    out.trim().to_string()
}
