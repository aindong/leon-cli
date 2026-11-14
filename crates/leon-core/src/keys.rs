//! Key names in binding syntax (`enter`, `ctrl+c`, `alt+left`, `a`) to the bytes an xterm-style
//! terminal sends for them. Printable keys use the US layout; names are case-insensitive.

/// The bytes for one key. `app_cursor` is the terminal's application-cursor mode (DECCKM).
pub fn encode(key: &str, app_cursor: bool) -> Result<Vec<u8>, String> {
    let bad = || format!("unknown key {key:?}");
    // A lone character types itself, case and all (`A`, `+`); only names and modifiers fold case.
    let mut chars = key.trim().chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return if c.is_control() { Err(bad()) } else { Ok(c.to_string().into_bytes()) };
    }
    let lower = key.trim().to_lowercase();
    let mut parts: Vec<&str> = lower.split('+').collect();
    let name = parts.pop().filter(|n| !n.is_empty()).ok_or_else(bad)?;
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    for m in parts {
        match m {
            "ctrl" | "control" => ctrl = true,
            "alt" | "opt" | "option" | "meta" => alt = true,
            "shift" => shift = true,
            _ => return Err(bad()),
        }
    }
    // Cursor keys carry modifiers as an xterm parameter: 1 + shift + 2·alt + 4·ctrl.
    let mods = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
    let cursor = |c: char| match (mods, app_cursor) {
        (1, true) => format!("\x1bO{c}"),
        (1, false) => format!("\x1b[{c}"),
        _ => format!("\x1b[1;{mods}{c}"),
    };
    let tilde = |n: u8| format!("\x1b[{n}~");
    let named = match name {
        "up" => Some(cursor('A')),
        "down" => Some(cursor('B')),
        "right" => Some(cursor('C')),
        "left" => Some(cursor('D')),
        "home" => Some(cursor('H')),
        "end" => Some(cursor('F')),
        _ => None,
    };
    if let Some(seq) = named {
        return Ok(seq.into_bytes());
    }
    let mut out = match name {
        "enter" | "return" => b"\r".to_vec(),
        "escape" | "esc" => b"\x1b".to_vec(),
        "tab" if shift => b"\x1b[Z".to_vec(),
        "tab" => b"\t".to_vec(),
        "backspace" => b"\x7f".to_vec(),
        "space" if ctrl => vec![0],
        "space" => b" ".to_vec(),
        "insert" => tilde(2).into_bytes(),
        "delete" => tilde(3).into_bytes(),
        "pageup" => tilde(5).into_bytes(),
        "pagedown" => tilde(6).into_bytes(),
        "f1" => b"\x1bOP".to_vec(),
        "f2" => b"\x1bOQ".to_vec(),
        "f3" => b"\x1bOR".to_vec(),
        "f4" => b"\x1bOS".to_vec(),
        f if f.len() > 1 && f.starts_with('f') => {
            let codes = [15, 17, 18, 19, 20, 21, 23, 24]; // f5..f12
            let n: usize = f[1..].parse().map_err(|_| bad())?;
            tilde(*codes.get(n.wrapping_sub(5)).ok_or_else(bad)?).into_bytes()
        }
        c if c.chars().count() == 1 => {
            let c = c.chars().next().unwrap();
            if ctrl {
                vec![control(c).ok_or_else(bad)?]
            } else if shift {
                shifted(c).ok_or_else(bad)?.to_string().into_bytes()
            } else if c.is_control() {
                return Err(bad());
            } else {
                c.to_string().into_bytes()
            }
        }
        _ => return Err(bad()),
    };
    if alt {
        out.insert(0, 0x1b);
    }
    Ok(out)
}

fn control(c: char) -> Option<u8> {
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1),
        '@' | ' ' | '2' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '/' => Some(0x1f),
        '?' => Some(0x7f),
        _ => None,
    }
}

/// What shift makes of a key on a US keyboard.
fn shifted(c: char) -> Option<char> {
    if c.is_ascii_lowercase() {
        return Some(c.to_ascii_uppercase());
    }
    let (plain, shift) = ("`1234567890-=[]\\;',./", "~!@#$%^&*()_+{}|:\"<>?");
    plain.chars().position(|p| p == c).and_then(|i| shift.chars().nth(i))
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn letters_that_look_like_function_keys() {
        assert_eq!(encode("f", false).unwrap(), b"f");
        assert_eq!(encode("f5", false).unwrap(), b"\x1b[15~");
        assert_eq!(encode("shift+1", false).unwrap(), b"!");
        assert_eq!(encode("ctrl+left", false).unwrap(), b"\x1b[1;5D");
        assert!(encode("f13", false).is_err());
    }
}
