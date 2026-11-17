//! How long a client retries a dropped link (`leon events -H`, `leon attach -H`, the app's hosts):
//! the server's `HOLD` (60 s) unless a test sets `LEON_LINK_HOLD_SECS`.

use std::time::Duration;

use leon_server::resume::{link_hold, HOLD};

#[test]
fn a_link_is_retried_for_the_servers_hold_unless_a_test_shortens_it() {
    std::env::remove_var("LEON_LINK_HOLD_SECS");
    assert_eq!(HOLD, Duration::from_secs(60));
    assert_eq!(link_hold(), HOLD);
    std::env::set_var("LEON_LINK_HOLD_SECS", "3");
    assert_eq!(link_hold(), Duration::from_secs(3));
}
