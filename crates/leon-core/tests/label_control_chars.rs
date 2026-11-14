#[test]
fn labels_reject_terminal_escapes() {
    assert!(leon_core::model::clean_label(Some("\u{1b}]0;pwned\u{7}")).is_err(), "LABEL_ACCEPTS_ESCAPES");
}
