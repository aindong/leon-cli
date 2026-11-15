//! Daily-driver A2: an Activity row is titled as the user knows the block (its label, the title
//! its program set, else the agent's OSC 7501 title), not by the program's name; its second line
//! ends at a word with "…" when it runs long.

mod m13_support;

use std::time::Instant;

use leon_app::attention::{cut_words, Tracker};
use leon_core::program_status::{Kind, Record, State};
use m13_support::*;

fn titled(title: &str, msg: Option<&str>) -> Record {
    Record { title: Some(title.into()), msg: msg.map(String::from), ..blocked(Kind::Question) }
}

const FSI: char = '\u{2068}';
const PDI: char = '\u{2069}';

#[test]
fn a_row_is_titled_by_label_then_program_title_then_status_title() {
    let mut m = model(vec![(
        "web",
        vec![
            Tab(None, "Fix the tests", vec![titled("Running", Some("step 2"))]),
            Tab(None, "", vec![titled("Run it on production now?", Some("the users table"))]),
            Tab(None, "", vec![record(State::Error)]),
            Tab(None, "Release notes", vec![record(State::Done)]),
        ],
    )]);
    let s = &mut m.sessions[0];
    for b in &mut s.blocks[1..3] {
        b.title = None; // the program set no title of its own
        b.command = vec!["/usr/local/bin/fake-agent".into()];
    }
    s.blocks[3].label = Some("notes".into());
    let mut t = Tracker::default();
    t.update(&[(0, "This Mac", &m)], Instant::now());
    let rows = t.rows();
    let by_block = |id: &str| rows.iter().find(|r| r.block_id == id).unwrap().clone();

    let r = by_block("block:web/0");
    assert_eq!(r.title, "Fix the tests");
    assert_eq!(r.caption, Some(format!("{FSI}Running: step 2{PDI}")), "the status title stays in the caption");
    let r = by_block("block:web/1");
    assert_eq!(r.title, "Run it on production now?", "the agent's own title, not fake-agent");
    assert_eq!(r.caption, Some(format!("{FSI}the users table{PDI}")), "and not said twice");
    let r = by_block("block:web/2");
    assert_eq!(r.title, "fake-agent", "with nothing set, the tab's name as before");
    let r = by_block("block:web/3");
    assert_eq!(r.title, "notes", "a label first");
    assert!(r.place.starts_with("This Mac › web › "), "{}", r.place);
}

#[test]
fn a_long_line_ends_at_a_word_with_an_ellipsis() {
    let s = "blocked · apply the patch to main and run the tests";
    // the wrapper says the third line would start at "main"
    let end = s.find("main").unwrap();
    assert_eq!(cut_words(s, end), "blocked · apply the patch…");
    assert_eq!(cut_words(s, s.len()), s, "what fits is unchanged");
    assert_eq!(cut_words(s, s.len() + 10), s);

    let isolated = format!("error · {FSI}apply the patch to main{PDI}");
    let end = isolated.find("main").unwrap();
    assert_eq!(cut_words(&isolated, end), format!("error · {FSI}apply the patch…{PDI}"), "the isolate stays closed");

    let one_word = "supercalifragilisticexpialidocious";
    assert_eq!(cut_words(one_word, 10), "supercali…", "a word longer than the room is cut inside it");
}
