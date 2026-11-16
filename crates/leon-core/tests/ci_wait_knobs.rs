//! The test-only wait knobs (`LEON_LINK_HOLD_SECS`, `LEON_LINK_PING_SECS`, `LEON_SPAWN_WAIT_SECS`)
//! leave every default as it was: only a positive number of seconds replaces it.

use std::time::Duration;

use leon_core::secs_from_env;

#[test]
fn only_a_positive_number_of_seconds_replaces_the_default() {
    let (name, default) = ("LEON_TEST_KNOB_SECS", Duration::from_secs(60));
    std::env::remove_var(name);
    assert_eq!(secs_from_env(name, default), default, "unset");
    for kept in ["", "0", "-3", "x", "1.5"] {
        std::env::set_var(name, kept);
        assert_eq!(secs_from_env(name, default), default, "{kept:?}");
    }
    std::env::set_var(name, "3");
    assert_eq!(secs_from_env(name, default), Duration::from_secs(3));
}
