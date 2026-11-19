//! `leon config check` with `cursor_shader` set: on macOS it translates the file as the app
//! will and prints `cursor_shader: PATH: ok`, or the reason with the shader's own line and exit
//! 1; off macOS it says cursor shaders are macOS only for now and the config still passes.
//! `leon config get`/`set` read and write both settings.

mod support;

use std::process::Output;

use support::Env;

fn check(env: &Env, init: &str) -> Output {
    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, init).unwrap();
    env.cmd().env("LEON_CONFIG", &path).args(["config", "check"]).output().unwrap()
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

// macOS only: the cases that translate a shader are; off macOS nothing reads one.
#[cfg(target_os = "macos")]
const GOOD: &str = "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    fragColor = texture(iChannel0, fragCoord / iResolution.xy);\n}\n";

// macOS only: cursor shaders are translated on macOS only; the case below is the rest.
#[cfg(target_os = "macos")]
#[test]
fn a_good_shader_is_ok_and_a_bad_one_names_its_line() {
    let env = Env::new();
    let dir = env.dir.path();
    std::fs::create_dir_all(dir.join("shaders")).unwrap();
    std::fs::write(dir.join("shaders/good.glsl"), GOOD).unwrap();
    // relative to init.lua's folder
    let o = check(&env, "leon.set(\"cursor_shader\", \"shaders/good.glsl\")\n");
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let want = format!("cursor_shader: {}: ok", dir.join("shaders/good.glsl").display());
    assert!(text(&o).contains(&want), "{} should say {want:?}", text(&o));

    std::fs::write(
        dir.join("shaders/bad.glsl"),
        "void mainImage(out vec4 c, in vec2 p) {\n\n    c = undefined_thing;\n}\n",
    )
    .unwrap();
    let o = check(&env, "leon.set(\"cursor_shader\", \"shaders/bad.glsl\")\n");
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("cursor_shader: bad.glsl:3: "), "{}", text(&o));
    assert!(text(&o).contains("undefined_thing"), "{}", text(&o));

    let o = check(&env, "leon.set(\"cursor_shader\", \"shaders/none.glsl\")\n");
    assert_eq!(o.status.code(), Some(1), "a missing file fails: {}", text(&o));
    assert!(text(&o).contains("none.glsl"), "{}", text(&o));
}

// macOS only: `~` is the HOME the check runs with; the app resolves it the same way.
#[cfg(target_os = "macos")]
#[test]
fn tilde_is_home() {
    let env = Env::new();
    let home = env.dir.path().join("home");
    std::fs::create_dir_all(home.join("s")).unwrap();
    std::fs::write(home.join("s/good.glsl"), GOOD).unwrap();
    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, "leon.set(\"cursor_shader\", \"~/s/good.glsl\")\n").unwrap();
    let o = env.cmd().env("LEON_CONFIG", &path).env("HOME", &home).args(["config", "check"]).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains(&format!("{}: ok", home.join("s/good.glsl").display())), "{}", text(&o));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn off_macos_the_check_says_so_and_passes() {
    let env = Env::new();
    let o = check(&env, "leon.set(\"cursor_shader\", \"~/s/smear.glsl\")\n");
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("cursor_shader: cursor shaders are macOS only for now"), "{}", text(&o));
}

#[test]
fn unset_says_nothing_and_get_set_work() {
    let env = Env::new();
    let o = check(&env, "-- mine\n");
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(!text(&o).contains("cursor_shader"), "{}", text(&o));
    let path = env.dir.path().join("init.lua");
    let config = |args: &[&str]| env.cmd().env("LEON_CONFIG", &path).arg("config").args(args).output().unwrap();
    let o = config(&["set", "cursor_shader_duration", "0.4"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let o = config(&["set", "cursor_shader", "~/s/smear.glsl"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert_eq!(String::from_utf8_lossy(&config(&["get", "cursor_shader"]).stdout).trim(), "~/s/smear.glsl");
    assert_eq!(String::from_utf8_lossy(&config(&["get", "cursor_shader_duration"]).stdout).trim(), "0.4");
    let o = config(&["set", "cursor_shader_duration", "9"]);
    assert_eq!(o.status.code(), Some(1), "out of range: {}", text(&o));
    assert!(text(&o).contains("0.1") && text(&o).contains("5"), "{}", text(&o));
}
