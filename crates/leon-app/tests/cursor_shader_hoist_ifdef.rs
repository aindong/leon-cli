//! A global under a preprocessor conditional is hoisted with its conditional, or not at all.

// macOS only: cursor shaders are built on macOS only for now (naga is a macOS dependency).
#![cfg(target_os = "macos")]

use leon_app::cursor_shader::{translate, Mode};

#[test]
fn a_global_in_a_disabled_branch_is_not_assigned_in_main() {
    let src = "#ifdef LEON_NOT_DEFINED\nfloat k = iTime;\n#endif\nvoid mainImage(out vec4 c, in vec2 p) {\n    c = vec4(1.0);\n}\n";
    for mode in [Mode::Contract, Mode::Matte] {
        let r = translate(src, mode);
        assert!(r.is_ok(), "HOIST_IFDEF_BUG {mode:?}: valid GLSL refused: {:?}", r.err());
    }
}

#[test]
fn a_global_in_an_inactive_if_block_is_not_assigned_in_main() {
    let src = "#if 0\nfloat a = 1.0;\n#endif\nvoid mainImage(out vec4 c, in vec2 p) { c = vec4(0.0); }\n";
    for mode in [Mode::Contract, Mode::Matte] {
        let r = translate(src, mode);
        assert!(r.is_ok(), "HOIST_IFDEF_BUG {mode:?}: {r:?}");
    }
}

#[test]
fn each_branch_keeps_its_own_assignment() {
    use leon_app::cursor_shader::hoist_globals;
    let src = "#define A\n#ifdef A\nfloat k = 1.0;\n#else\nfloat k = 0.0;\n#endif\nvoid mainImage(out vec4 c, in vec2 p) { c = vec4(k); }\n";
    let (_, inits) = hoist_globals(src);
    assert_eq!(
        inits,
        vec![(3, "#ifdef A\nk = 1.0;\n#endif".to_string()), (5, "#ifdef A\n#else\nk = 0.0;\n#endif".to_string())]
    );
    assert!(translate(src, Mode::Matte).is_ok(), "{:?}", translate(src, Mode::Matte).err());
}

#[test]
fn a_continued_directive_stays_whole_and_a_declaration_keeps_its_line() {
    use leon_app::cursor_shader::hoist_globals;
    let src = "#define F(x) \\\n    float q = x;\nfloat a;\n\nvec4 C = vec4(1.0);\n";
    let (out, inits) = hoist_globals(src);
    assert!(out.starts_with("#define F(x) \\\n    float q = x;\n"), "the macro's body stays in it:\n{out}");
    assert_eq!(inits, vec![(5, "C = vec4(1.0);".to_string())], "only C is hoisted");
    assert_eq!(out.lines().nth(4), Some("vec4 C;"), "on its own line:\n{out}");
}
