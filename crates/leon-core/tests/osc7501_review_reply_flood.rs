use leon_core::program_status::{Tracker, QUERY_REPLY};

#[test]
fn query_replies_per_read_are_capped() {
    let flood = "\x1b]7501;?\x07".repeat(10_000);
    let fed = Tracker::default().feed(flood.as_bytes(), |_| {});
    assert!(
        fed.reply.len() <= 16 * QUERY_REPLY.len(),
        "{} bytes of queries produced {} reply bytes",
        flood.len(),
        fed.reply.len()
    );
}
