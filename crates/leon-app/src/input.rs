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

/// Pastes are wrapped by the server now (`block.event` paste), which knows the program's mode.
pub use leon_core::keys::paste_bytes;

/// GPUI rebuilds a key's text from its key code through the keyboard layout, so an event with key
/// code 0 and text of its own (what automation and some accessibility tools send) arrives as `a`.
/// When the event's `characters` disagree with what GPUI made, they are the key. A real key on
/// code 0 (`a` in QWERTY, `q` in AZERTY, with shift or option) agrees, and stays as it is; so
/// does one that differs only in case (Caps Lock, which GPUI leaves out of every key alike).
pub fn retext(ks: Keystroke, key_code: u16, characters: &str) -> Keystroke {
    let m = &ks.modifiers;
    let agrees = ks.key_char.as_deref().is_some_and(|c| c.to_lowercase() == characters.to_lowercase());
    if key_code != 0 || m.control || m.platform || characters.is_empty() || agrees {
        return ks;
    }
    let key = characters.chars().next().map(|c| c.to_lowercase().to_string()).unwrap_or_default();
    Keystroke { key, key_char: Some(characters.to_string()), ..ks }
}

/// The key code and text of the key event AppKit is dispatching now (`[NSApp currentEvent]`):
/// GPUI calls `key_down` from inside it. Main thread only (the real app's).
pub fn current_key_event() -> Option<(u16, String)> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    const KEY_DOWN: usize = 10; // NSEventTypeKeyDown
    let cls = AnyClass::get(c"NSApplication")?;
    unsafe {
        let app: *mut AnyObject = msg_send![cls, sharedApplication];
        let ev: *mut AnyObject = if app.is_null() { return None } else { msg_send![app, currentEvent] };
        if ev.is_null() {
            return None;
        }
        let kind: usize = msg_send![ev, type];
        if kind != KEY_DOWN {
            return None;
        }
        let code: u16 = msg_send![ev, keyCode];
        let text: *mut AnyObject = msg_send![ev, characters];
        let utf8: *const std::ffi::c_char = if text.is_null() { return None } else { msg_send![text, UTF8String] };
        if utf8.is_null() {
            return None;
        }
        Some((code, std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned()))
    }
}
