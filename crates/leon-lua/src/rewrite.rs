//! `leon config set`: change one setting in init.lua and keep every other byte. The call that set
//! it last is rewritten in place when it is `leon.set("KEY", <literal>)`; a key never set gets a
//! `leon.set` line at the end. The new text is loaded before it replaces the file, and only
//! replaces it when it loads with the new value and no new error. Anything else is refused with
//! the `file:line` to change by hand.

use std::io::Write;
use std::ops::Range;
use std::path::Path;

use serde_json::Value;

use crate::{Config, Level};

/// One write at a time in this process: each reads the file the last one left, and they share a
/// temp file name.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Set `key` to `value` in the config at `config` (created if missing). Returns where it is set now.
pub fn write_setting(config: &Path, key: &str, value: &Value) -> Result<String, String> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let shown = config.display().to_string();
    let exists = config.exists();
    let (src, before) = if exists {
        let src = std::fs::read_to_string(config).map_err(|e| format!("can't read {shown}: {e}"))?;
        let cfg = crate::load(config).map_err(|e| format!("{e}; fix it before `leon config set`"))?;
        (src, cfg)
    } else {
        (String::new(), Config::default())
    };
    // the value as the settings would hold it, or why it isn't one (checked before any rewrite)
    let mut want = before.settings.clone();
    want.set(key, value, &crate::settings::themes_dir(config))?;
    if let Some(why) = read_only(config, &src, &before.setting_sources, key) {
        return Err(why);
    }
    let new = match before.setting_sources.get(key) {
        None => append(&src, key, value),
        Some(at) => {
            let line = at.rsplit_once(':').and_then(|(_, l)| l.parse().ok()).unwrap_or(0);
            replace(&src, line, key, value).ok_or_else(|| format!("{key}: can't rewrite its call at {shown}:{line}"))?
        }
    };
    let after =
        crate::load_text(config, &new).map_err(|e| format!("the edited config wouldn't load ({e}); not written"))?;
    let errors = |c: &Config| c.diagnostics.iter().filter(|d| d.level == Level::Error).count();
    if after.settings.get(key) != want.get(key) || errors(&after) > errors(&before) {
        let why = after.diagnostics.iter().rev().find(|d| d.level == Level::Error).map(|d| format!(": {d}"));
        return Err(format!("the edit didn't set {key}{}; {shown} is unchanged", why.unwrap_or_default()));
    }
    save(config, exists, &new)?;
    Ok(after.setting_sources.get(key).cloned().unwrap_or(shown))
}

/// Why `write_setting` would refuse to change `key` in the config at `config` whose text is `src`
/// and whose `leon.set` calls are at `sources` (`Config::setting_sources`), or None when it can.
pub fn read_only(
    config: &Path,
    src: &str,
    sources: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Option<String> {
    let at = sources.get(key)?;
    let refuse =
        |why: &str| format!("{key} is set at {at}{why}; change it there (or remove that line and run this again)");
    let (file, line) = at.rsplit_once(':').unwrap_or_default();
    if file != config.display().to_string() {
        return Some(refuse(", in a file other than the config"));
    }
    let line = line.parse().unwrap_or(0);
    // any value will do: only whether the call is one `replace` understands matters
    match replace(src, line, key, &Value::Bool(true)) {
        Some(_) => None,
        None => Some(refuse(" by code `leon config set` can't rewrite: it changes only leon.set(\"KEY\", <literal>)")),
    }
}

/// Write `text` over the file `config` resolves to (a symlink stays one), through a temp file
/// beside it, with the old file's permissions.
fn save(config: &Path, exists: bool, text: &str) -> Result<(), String> {
    let err = |e: std::io::Error| format!("can't write {}: {e}", config.display());
    // follow the links (dotfiles), even to a target not made yet, so the link survives the rename
    let mut real = config.to_path_buf();
    for _ in 0..16 {
        let Ok(target) = std::fs::read_link(&real) else { break };
        real = real.parent().unwrap_or(Path::new(".")).join(target);
    }
    let dir = real.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(err)?;
    let name = real.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.leon-{}", std::process::id()));
    // create_new never follows or reuses what is already at `tmp`, so only a file made here is removed
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(err)?;
    let written = (|| {
        if exists {
            f.set_permissions(std::fs::metadata(&real)?.permissions())?;
        }
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, &real)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written.map_err(err)
}

fn append(src: &str, key: &str, value: &Value) -> String {
    let gap = if src.is_empty() || src.ends_with('\n') { "" } else { "\n" };
    format!("{src}{gap}leon.set({}, {})\n", string(key), literal(value))
}

/// `value` as Lua source.
pub fn literal(value: &Value) -> String {
    match value {
        Value::String(s) => string(s),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        Value::Object(o) => {
            let name = |k: &str| match k.chars().next() {
                Some(c) if !c.is_ascii_digit() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => k.into(),
                _ => format!("[{}]", string(k)),
            };
            let fields: Vec<String> = o.iter().map(|(k, v)| format!("{} = {}", name(k), literal(v))).collect();
            format!("{{ {} }}", fields.join(", "))
        }
        Value::Array(list) if list.is_empty() => "{}".into(),
        Value::Array(list) => format!("{{ {} }}", list.iter().map(literal).collect::<Vec<_>>().join(", ")),
        v => v.to_string(), // true, false
    }
}

fn string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '\\' => out.extend(['\\', c]),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 32 || c == '\u{7f}' => out += &format!("\\{:03}", c as u32),
            c => out.push(c),
        }
    }
    out + "\""
}

/// `src` with the value of the one `leon.set(KEY, …)` call starting on `line` (1-based) replaced,
/// or None when that call's key or value isn't a literal this understands.
pub fn replace(src: &str, line: usize, key: &str, value: &Value) -> Option<String> {
    let start = if line <= 1 { 0 } else { src.match_indices('\n').nth(line - 2)?.0 + 1 };
    let end = src[start..].find('\n').map_or(src.len(), |i| start + i);
    let text = &src[start..end];
    let code = code_len(text);
    let calls: Vec<Call> = text
        .match_indices("leon.set")
        .filter(|(i, _)| *i < code && !text[..*i].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '.'))
        .filter_map(|(i, _)| call(src, start + i + "leon.set".len()))
        .filter(|c| c.key == key)
        .collect();
    let [c] = calls.as_slice() else { return None };
    let mut edits: Vec<(Range<usize>, String)> = match (&c.fields, value) {
        // a table to a table: change just the names, so comments inside it stay
        (Some(f), Value::Object(o)) if f.len() == o.len() && f.iter().all(|(k, _)| o.contains_key(k)) => {
            f.iter().map(|(k, span)| (span.clone(), literal(&o[k]))).collect()
        }
        _ => vec![(c.value.clone(), literal(value))],
    };
    edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
    let mut out = src.to_string();
    for (r, text) in edits {
        out.replace_range(r, &text);
    }
    Some(out)
}

/// How much of `line` is code: up to a `--` comment outside a string.
fn code_len(line: &str) -> usize {
    let (mut quote, mut escaped) = (None, false);
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (Some(_), _) if escaped => escaped = false,
            (Some(_), '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, '-') if line[i..].starts_with("--") => return i,
            _ => {}
        }
    }
    line.len()
}

/// A table literal's fields: each name and the span of its string value.
type Fields = Vec<(String, Range<usize>)>;

/// A parsed `leon.set("key", value)`: the value's span and, for a table of names, each field's.
struct Call {
    key: String,
    value: Range<usize>,
    fields: Option<Fields>,
}

/// The call whose `(` follows `at`, if its key is a string and its value a literal.
fn call(src: &str, at: usize) -> Option<Call> {
    let b = src.as_bytes();
    let mut i = skip(src, at);
    (b.get(i) == Some(&b'(')).then_some(())?;
    let (key, k) = lua_string(src, skip(src, i + 1))?;
    i = skip(src, k.end);
    (b.get(i) == Some(&b',')).then_some(())?;
    i = skip(src, i + 1);
    let (value, fields) = match b.get(i)? {
        b'"' | b'\'' => (lua_string(src, i)?.1, None),
        b'{' => {
            let (span, fields) = table(src, i)?;
            (span, Some(fields))
        }
        _ => {
            let len =
                src[i..].find(|c: char| !(c.is_ascii_alphanumeric() || ".+-".contains(c))).unwrap_or(src.len() - i);
            let word = &src[i..i + len];
            (word == "true" || word == "false" || word.parse::<f64>().is_ok()).then_some(())?;
            (i..i + len, None)
        }
    };
    (b.get(skip(src, value.end)) == Some(&b')')).then_some(())?;
    Some(Call { key, value, fields })
}

/// `{ name = "string", … }` or `{ "string", … }` from the `{` at `at`: its span and each field's
/// string span (a list's fields have no name).
fn table(src: &str, at: usize) -> Option<(Range<usize>, Fields)> {
    let b = src.as_bytes();
    let (mut i, mut fields) = (skip(src, at + 1), vec![]);
    while b.get(i)? != &b'}' {
        let mut name = String::new();
        if !matches!(b[i], b'"' | b'\'') {
            let len = src[i..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
            (len > 0).then_some(())?;
            name = src[i..i + len].to_string();
            i = skip(src, i + len);
            (b.get(i) == Some(&b'=')).then_some(())?;
            i = skip(src, i + 1);
        }
        let (_, span) = lua_string(src, i)?;
        i = skip(src, span.end);
        fields.push((name, span));
        match b.get(i)? {
            b',' | b';' => i = skip(src, i + 1),
            b'}' => {}
            _ => return None,
        }
    }
    Some((at..i + 1, fields))
}

/// A short string literal at `at` with simple escapes: its value and span.
fn lua_string(src: &str, at: usize) -> Option<(String, Range<usize>)> {
    let q = *src.as_bytes().get(at)?;
    (q == b'"' || q == b'\'').then_some(())?;
    let (mut out, mut chars) = (String::new(), src[at + 1..].char_indices());
    while let Some((i, c)) = chars.next() {
        match c {
            '\n' => return None,
            '\\' => match chars.next()?.1 {
                e @ ('"' | '\'' | '\\') => out.push(e),
                'n' => out.push('\n'),
                't' => out.push('\t'),
                _ => return None,
            },
            c if c as u32 == q as u32 => return Some((out, at..at + 1 + i + 1)),
            c => out.push(c),
        }
    }
    None
}

/// Past whitespace and `--` comments from `at` (a long `--[[ ]]` comment ends the parse).
fn skip(src: &str, mut at: usize) -> usize {
    loop {
        let rest = &src[at.min(src.len())..];
        let trimmed = rest.trim_start();
        at += rest.len() - trimmed.len();
        if trimmed.starts_with("--") && !trimmed.starts_with("--[") {
            at += trimmed.find('\n').unwrap_or(trimmed.len());
        } else {
            return at;
        }
    }
}
