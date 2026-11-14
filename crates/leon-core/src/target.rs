//! Target resolution, in the order docs/spec.md §2 gives: full ID, then case-insensitive label,
//! then 1-based position, then an ID suffix of at least 4 characters.

use crate::ApiError;

/// Something a query can name: its ID and optional label, in list order.
pub struct Candidate<'a> {
    pub id: &'a str,
    pub label: Option<&'a str>,
}

/// The index of the one candidate `query` names.
pub fn resolve(kind: &str, query: &str, items: &[Candidate]) -> Result<usize, ApiError> {
    let q = query.trim();
    if let Some(i) = items.iter().position(|c| c.id == q) {
        return Ok(i);
    }
    if let Some(i) = one(kind, q, items, |c| c.label.is_some_and(|l| l.eq_ignore_ascii_case(q)))? {
        return Ok(i);
    }
    if let Ok(n) = q.parse::<usize>() {
        if (1..=items.len()).contains(&n) {
            return Ok(n - 1);
        }
    }
    if q.chars().count() >= 4 {
        if let Some(i) = one(kind, q, items, |c| c.id.ends_with(q))? {
            return Ok(i);
        }
    }
    Err(ApiError::not_found(format!("no {kind} matches {q:?}")))
}

/// The single candidate matching `pred`, none, or an ambiguity error listing every match.
fn one(kind: &str, q: &str, items: &[Candidate], pred: impl Fn(&Candidate) -> bool) -> Result<Option<usize>, ApiError> {
    let hits: Vec<usize> = (0..items.len()).filter(|&i| pred(&items[i])).collect();
    match hits.as_slice() {
        [] => Ok(None),
        [i] => Ok(Some(*i)),
        _ => Err(ApiError::ambiguous(
            &format!("{kind} {q}"),
            &hits.iter().map(|&i| items[i].id.to_string()).collect::<Vec<_>>(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c<'a>(id: &'a str, label: Option<&'a str>) -> Candidate<'a> {
        Candidate { id, label }
    }

    #[test]
    fn order_id_label_position_suffix() {
        let items = [c("session:aaaa1111", Some("web")), c("session:bbbb2222", Some("1")), c("session:cccc3333", None)];
        assert_eq!(resolve("session", "session:cccc3333", &items).unwrap(), 2, "full id");
        assert_eq!(resolve("session", "WEB", &items).unwrap(), 0, "label, case-insensitive");
        assert_eq!(resolve("session", "1", &items).unwrap(), 1, "label beats position");
        assert_eq!(resolve("session", "3", &items).unwrap(), 2, "position");
        assert_eq!(resolve("session", "3333", &items).unwrap(), 2, "suffix");
    }

    #[test]
    fn misses_and_ambiguity() {
        let items = [c("session:aaaa1111", Some("x")), c("session:bbbb1111", Some("x"))];
        assert_eq!(resolve("session", "nope", &items).unwrap_err().code, "not_found");
        assert_eq!(resolve("session", "111", &items).unwrap_err().code, "not_found", "suffix needs 4 chars");
        let e = resolve("session", "x", &items).unwrap_err();
        assert_eq!(e.code, "ambiguous");
        assert!(e.message.contains("session:aaaa1111") && e.message.contains("session:bbbb1111"));
        assert_eq!(resolve("session", "1111", &items).unwrap_err().code, "ambiguous", "ambiguous suffix");
        assert_eq!(resolve("session", "0", &items).unwrap_err().code, "not_found", "positions start at 1");
    }
}
