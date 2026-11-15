use super::*;

fn session(label: &str) -> Session {
    Session {
        id: format!("session:{label}"),
        label: Some(label.into()),
        revision: 0,
        active: 0,
        windows: Vec::new(),
        blocks: Vec::new(),
        overlays: Default::default(),
    }
}

#[test]
fn the_picker_draws_at_any_size() {
    let rows = rows(&[session("web"), session("api")]);
    let p = Picker::default();
    for w in 0..60 {
        for h in 0..8 {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            p.draw(&mut buf, area, &rows, &HashMap::new(), Colors { truecolor: false });
        }
    }
}

#[test]
fn the_filter_is_fuzzy_and_best_first() {
    let rows = rows(&[session("alpha-one"), session("beta-two")]);
    let p = Picker { query: "btwo".into(), selected: 0 };
    let shown: Vec<&str> = p.shown(&rows).iter().map(|&i| rows[i].path.as_str()).collect();
    assert_eq!(shown, ["beta-two"]);
    let p = Picker::default();
    assert_eq!(p.shown(&rows).len(), 2, "no query keeps every row");
}
