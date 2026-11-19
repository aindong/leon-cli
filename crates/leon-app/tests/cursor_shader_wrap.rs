//! The contract wrapper: the prefix goes before the user's file, every line of the file keeps its
//! number (so naga's errors point at the user's own line), and top-level variables with an
//! initializer are hoisted into `main`, leaving `const`s, functions and what it can't read alone.

// macOS only: cursor shaders are built on macOS only for now (naga is a macOS dependency).
#![cfg(target_os = "macos")]

use leon_app::cursor_shader::{hoist_globals, load, read, translate, wrap, Mode, MAX_BYTES, PREFIX};

#[test]
fn the_prefix_comes_first_and_the_users_lines_keep_their_numbers() {
    let src = "// line 1\nvoid mainImage(out vec4 c, in vec2 p) {\n    c = vec4(1.0);\n}\n";
    for mode in [Mode::Contract, Mode::Matte] {
        let w = wrap(src, mode);
        assert!(w.text.starts_with(PREFIX), "{mode:?}");
        for (n, line) in src.lines().enumerate() {
            let at = w.text.lines().position(|l| l == line).unwrap() as u32 + 1;
            assert_eq!(w.user_line(at), Some(n as u32 + 1), "{mode:?}: {line:?}");
        }
        assert_eq!(w.user_line(1), None, "the prefix's lines are leon's");
    }
    for want in ["iCurrentCursor", "iPreviousCursor", "iTimeCursorChange", "CURSORSTYLE_BAR", "iChannel0", "texture2D"]
    {
        assert!(PREFIX.contains(want), "the contract has {want}");
    }
}

#[test]
fn a_global_with_an_initializer_is_hoisted_and_keeps_its_lines() {
    let src = "#define K 2.0\nconst float A = 1.0;\nvec4 C = vec4(\n    iCurrentCursorColor.rgb,\n    1.0);\nfloat f() { float x = 1.0; return x; }\nvoid mainImage(out vec4 c, in vec2 p) { c = C; }\n";
    let (out, inits) = hoist_globals(src);
    assert_eq!(out.lines().count(), src.lines().count(), "the same number of lines:\n{out}");
    assert!(out.contains("const float A = 1.0;"), "a const stays");
    assert!(out.contains("vec4 C;\n\n\n"), "the declaration is bare, padded to its lines:\n{out}");
    assert!(out.contains("float f() { float x = 1.0; return x; }"), "a function body is not top level");
    assert!(out.starts_with("#define K 2.0\n"), "a directive stays");
    assert_eq!(inits, vec![(3, "C = vec4( iCurrentCursorColor.rgb, 1.0);".to_string())]);
}

#[test]
fn what_it_cant_read_is_left_for_naga() {
    for src in [
        "float a = 1.0, b = 2.0;\n",          // two declarators
        "float a[2] = float[2](1.0, 2.0);\n", // an array
        "uniform float u;\n",                 // no initializer
        "highp float h = 1.0;\n",             // a qualifier first
        "struct S { float x; };\n",
    ] {
        let (out, inits) = hoist_globals(src);
        assert_eq!(out, src, "unchanged");
        assert!(inits.is_empty(), "{src}");
    }
}

#[test]
fn comments_with_braces_and_semicolons_dont_confuse_it() {
    let src = "/* { not code; */\n// } nor this;\nvec2 P = iResolution.xy * 0.5; // the middle\nvoid mainImage(out vec4 c, in vec2 p) { /* } */ c = vec4(P, 0.0, 1.0); }\n";
    let (out, inits) = hoist_globals(src);
    assert_eq!(inits, vec![(3, "P = iResolution.xy * 0.5;".to_string())]);
    assert_eq!(out.lines().count(), src.lines().count());
    assert!(translate(src, Mode::Matte).is_ok(), "{:?}", translate(src, Mode::Matte));
}

#[test]
fn an_error_names_the_users_line() {
    let src = "void mainImage(out vec4 c, in vec2 p) {\n    c = vec4(1.0);\n    c = nope;\n}\n";
    let e = translate(src, Mode::Matte).unwrap_err();
    assert_eq!(e.line, Some(3), "{e}");
    assert!(e.to_string().starts_with("line 3: "), "{e}");
    assert!(e.message.contains("nope"), "{e}");
    // in a hoisted initializer, which naga sees inside main: still the user's line
    let src = "\n\nvec4 C = vec4(nothing_here, 1.0);\nvoid mainImage(out vec4 c, in vec2 p) { c = C; }\n";
    assert_eq!(translate(src, Mode::Matte).unwrap_err().line, Some(3));
    // no mainImage at all
    assert!(translate("float x;\n", Mode::Matte).is_err());
}

#[test]
fn load_reads_and_says_the_file_and_line() {
    let d = tempfile::tempdir().unwrap();
    let bad = d.path().join("bad.glsl");
    std::fs::write(&bad, "void mainImage(out vec4 c, in vec2 p) {\n    c = nope;\n}\n").unwrap();
    let e = load(&bad).unwrap_err();
    assert!(e.starts_with("bad.glsl:2: "), "{e}");
    let good = d.path().join("good.glsl");
    std::fs::write(&good, "void mainImage(out vec4 c, in vec2 p) { c = vec4(0.0); }\n").unwrap();
    assert!(load(&good).unwrap().contains("leon_vertex"));
    let missing = d.path().join("missing.glsl");
    assert!(read(&missing).unwrap_err().contains("missing.glsl"));
    assert!(read(d.path()).unwrap_err().contains("not a file"));
    let big = d.path().join("big.glsl");
    std::fs::write(&big, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
    assert!(read(&big).unwrap_err().contains("bytes"), "a file past the limit isn't read");
}
