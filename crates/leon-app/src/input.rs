//! Keystrokes and pastes to the bytes a terminal program reads.

use gpui::Keystroke;

/// The bytes for a keystroke, or None for one the terminal doesn't get (cmd shortcuts, unknown
/// keys). Typed characters go as typed, so option compositions (`ß`) work; everything else is
/// spelled in binding syntax and encoded by the table `send-key` uses.
pub fn key_bytes(ks: &Keystroke, app_cursor: bool) -> Option<Vec<u8>> {
    let m = &ks.modifiers;
    if m.platform {
        return None;
    }
    let single = ks.key.chars().count() == 1;
    if !m.control && (single || ks.key == "space") {
        if let Some(typed) = ks.key_char.as_deref().filter(|c| !c.is_empty() && !c.chars().any(char::is_control)) {
            return Some(typed.as_bytes().to_vec());
        }
    }
    let mut name = String::new();
    for (on, prefix) in [(m.control, "ctrl+"), (m.alt, "alt+"), (m.shift, "shift+")] {
        if on {
            name.push_str(prefix);
        }
    }
    name.push_str(&ks.key);
    leon_core::keys::encode(&name, app_cursor).ok()
}

/// Pasted text as the shell should receive it: line breaks become Enter (`\r`). In bracketed
/// paste mode the text is wrapped in the paste markers, with ESC removed so the text can't
/// close the bracket itself and run what follows.
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if !bracketed {
        return text.into_bytes();
    }
    let mut out = b"\x1b[200~".to_vec();
    out.extend(text.bytes().filter(|&b| b != 0x1b));
    out.extend_from_slice(b"\x1b[201~");
    out
}
