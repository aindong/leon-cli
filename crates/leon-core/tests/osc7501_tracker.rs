//! Tracker: scanner + store per terminal, end to end, split byte by byte.

use leon_core::program_status::{Change, Reason, Record, State, Tracker, QUERY_REPLY};

fn rec(state: State) -> Record {
    Record { state, kind: None, progress: None, app: None, title: None, msg: None }
}

fn changed(id: &str, state: State) -> Change {
    Change::Changed { id: Some(id.into()), record: rec(state), reason: Reason::Report }
}

/// Returns (pass-through bytes, changes, reply bytes).
fn run(chunks: &[&[u8]]) -> (Vec<u8>, Vec<Change>, Vec<u8>) {
    let mut t = Tracker::default();
    let (mut pass, mut changes, mut reply) = (Vec::new(), Vec::new(), Vec::new());
    for chunk in chunks {
        let fed = t.feed(chunk, |b| pass.extend_from_slice(b));
        changes.extend(fed.changes);
        reply.extend(fed.reply);
    }
    assert_eq!(t.store.records(), vec![(Some("c".to_string()), rec(State::Error))], "only c survives");
    (pass, changes, reply)
}

#[test]
fn query_reply_is_the_spec_string() {
    assert_eq!(QUERY_REPLY, b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\");
}

#[test]
fn stream_split_byte_by_byte_matches_one_read() {
    let stream = concat!(
        "pre",
        "\x1b]7501;state=working:id=a\x07",
        "\x1b]7501;?\x1b\\",
        "\x1b]133;A\x07",
        "\x1b]7501;state=done:id=b\x07",
        "\x1bc",
        "\x1b]7501;state=error:id=c\x07",
        "\x1b]7501;state=bogus:id=d\x07",
        "post",
    )
    .as_bytes();
    let want_changes = vec![
        changed("a", State::Working),
        Change::Removed { ids: vec![Some("a".into())], reason: Reason::Prompt },
        changed("b", State::Done),
        Change::Removed { ids: vec![Some("b".into())], reason: Reason::Reset },
        changed("c", State::Error),
    ];
    let want = (b"pre\x1b]133;A\x07\x1bcpost".to_vec(), want_changes, QUERY_REPLY.to_vec());
    assert_eq!(run(&[stream]), want, "one read");
    let singles: Vec<&[u8]> = stream.chunks(1).collect();
    assert_eq!(run(&singles), want, "byte by byte");
}
