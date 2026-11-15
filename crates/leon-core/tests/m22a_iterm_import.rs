//! M22a criterion 3 (.agent/plans/2026-11-15-m22a-themes-settings.md): iTerm2 .itermcolors (XML
//! plists) parse into a theme; malformed ones are clear errors, never a panic.

use leon_core::theme::import::parse;

fn colour(name: &str, r: &str, g: &str, b: &str) -> String {
    format!(
        "\t<key>{name}</key>\n\t<dict>\n\t\t<key>Alpha Component</key>\n\t\t<real>1</real>\n\
         \t\t<key>Blue Component</key>\n\t\t<real>{b}</real>\n\t\t<key>Color Space</key>\n\
         \t\t<string>sRGB</string>\n\t\t<key>Green Component</key>\n\t\t<real>{g}</real>\n\
         \t\t<key>Red Component</key>\n\t\t<real>{r}</real>\n\t</dict>\n"
    )
}

fn plist(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n{body}</dict>\n</plist>\n"
    )
}

/// A theme written for this test: grays and a few primaries.
fn sample() -> String {
    let mut body = String::new();
    for i in 0..16 {
        let v = format!("{}", i as f64 / 15.0);
        body += &colour(&format!("Ansi {i} Color"), &v, &v, &v);
    }
    body += &colour("Background Color", "0", "0", "0.2");
    body += &colour("Foreground Color", "1", "1", "1");
    body += &colour("Cursor Color", "1", "0.5", "0");
    body += &colour("Selection Color", "0.2", "0.4", "0.6");
    body += &colour("Bold Color", "1", "1", "1");
    plist(&body)
}

#[test]
fn an_itermcolors_file_becomes_a_theme() {
    let t = parse(sample().as_bytes()).unwrap();
    assert_eq!(t.bg, 0x000033);
    assert_eq!(t.fg, 0xFFFFFF);
    assert_eq!(t.cursor, 0xFF8000, "0.5 rounds to 128");
    assert_eq!(t.selection, 0x336699);
    assert_eq!(t.ansi[0], 0x000000);
    assert_eq!(t.ansi[15], 0xFFFFFF);
    assert_eq!(t.ansi[5], 0x555555, "5/15 of 255 is 85");
    assert!(t.is_dark());
}

#[test]
fn integer_components_work_too() {
    let body = colour("Background Color", "0", "0", "0").replace("<real>0</real>", "<integer>0</integer>")
        + &colour("Foreground Color", "1", "1", "1");
    let t = parse(plist(&body).as_bytes()).unwrap();
    assert_eq!((t.bg, t.fg), (0, 0xFFFFFF));
}

#[test]
fn per_appearance_keys_fill_in_for_missing_plain_ones() {
    let body = colour("Background Color (Light)", "1", "1", "1")
        + &colour("Background Color (Dark)", "0", "0", "0")
        + &colour("Foreground Color", "1", "1", "1")
        + &colour("Foreground Color (Dark)", "0.5", "0.5", "0.5")
        + &colour("Ansi 1 Color (Dark)", "1", "0", "0");
    let t = parse(plist(&body).as_bytes()).unwrap();
    assert_eq!(t.bg, 0, "the dark background stands in, the light one is left out");
    assert_eq!(t.fg, 0xFFFFFF, "a plain key wins over its dark one");
    assert_eq!(t.ansi[1], 0xFF0000);
}

#[test]
fn malformed_plists_are_clear_errors() {
    let fg = colour("Foreground Color", "1", "1", "1");
    let bg = colour("Background Color", "0", "0", "0");
    let bomb = "<?xml version=\"1.0\"?>\n<!DOCTYPE plist [\n<!ENTITY a \"aaaaaaaaaa\">\n\
        <!ENTITY b \"&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;\">\n<!ENTITY c \"&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;\">\n\
        <!ENTITY d \"&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;\">\n<!ENTITY e \"&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;\">\n\
        <!ENTITY f \"&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;\">\n<!ENTITY g \"&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;\">\n\
        <!ENTITY h \"&g;&g;&g;&g;&g;&g;&g;&g;&g;&g;\">\n]>\n<plist><dict><key>&h;</key><dict/></dict></plist>\n"
        .to_string();
    let cases: Vec<(String, &str)> = vec![
        ("<?xml version=\"1.0\"?><plist><dict><key>x</key>".into(), "XML"),
        ("bplist00\u{1}\u{2}".into(), "binary plist"),
        (plist(&fg), "Background Color"),
        (plist(&bg), "Foreground Color"),
        (plist(&(bg.clone() + &colour("Foreground Color", "2.5", "1", "1"))), "Foreground Color"),
        (plist(&(bg.clone() + &colour("Foreground Color", "NaN", "1", "1"))), "Foreground Color"),
        (plist(&(bg.clone() + &colour("Foreground Color", "one", "1", "1"))), "Foreground Color"),
        (plist(&(bg.clone() + &fg.replace("<key>Red Component</key>", "<key>Rouge</key>"))), "Red Component"),
        (plist(&(bg.clone() + &fg + &colour("Ansi 99 Color", "1", "1", "1"))), "Ansi 99 Color"),
        (plist(&(bg.clone() + "\t<key>Foreground Color</key>\n\t<string>white</string>\n")), "Foreground Color"),
        (plist(&(bg.clone() + "\t<key>Dangling</key>\n")), "Dangling"),
        ("<?xml version=\"1.0\"?><plist><array/></plist>".into(), "dict"),
        ("<?xml version=\"1.0\"?><html><body/></html>".into(), "plist"),
        (bomb, "XML"),
    ];
    for (input, want) in cases {
        let shown = input.chars().take(80).collect::<String>();
        let err = match std::panic::catch_unwind(|| parse(input.as_bytes())) {
            Ok(Err(e)) => e,
            Ok(Ok(t)) => panic!("{shown:?} parsed into {t:?}"),
            Err(_) => panic!("{shown:?} panicked"),
        };
        assert!(err.contains(want), "{shown:?}: error {err:?} should mention {want:?}");
    }
}
