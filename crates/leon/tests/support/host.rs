//! A host terminal for `leon attach` tests: a real PTY with an emulator behind it (leon's own
//! `Terminal`, portable-pty plus alacritty), running a shell script with this env's leon.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alacritty_terminal::term::TermMode;
use leon_core::terminal::{Notice, Spec, Terminal};

use super::Env;

pub struct Host {
    pub term: Terminal,
    exit: Arc<Mutex<Option<i32>>>,
}

impl Host {
    /// Run `script` under `/bin/sh -c` in a fresh 24×80 PTY, with `$LEON` naming the leon binary
    /// and the env's private runtime and state dirs. `vars` are set on top (`COLORTERM=…`).
    pub fn spawn(env: &Env, script: &str, vars: &[(&str, &str)]) -> Host {
        let mut command: Vec<String> = ["/usr/bin/env", "-u", "LEON_SESSION", "-u", "LEON_BLOCK", "-u", "LEON_SERVER"]
            .into_iter()
            .chain(["-u", "LEON_AUTOSTART", "-u", "LEON_IDLE_EXIT_SECS", "-u", "COLORTERM"])
            .map(String::from)
            .collect();
        let dir = env.dir.path();
        command.extend([
            format!("LEON_RUNTIME_DIR={}", dir.display()),
            format!("LEON_STATE_DIR={}", env.state_dir().display()),
            format!("LEON_CONFIG={}", dir.join("no-init.lua").display()),
            format!("LEON={}", env!("CARGO_BIN_EXE_leon")),
            "SHELL=/bin/sh".into(),
            "TERM=xterm-256color".into(),
        ]);
        command.extend(vars.iter().map(|(k, v)| format!("{k}={v}")));
        command.extend(["/bin/sh".into(), "-c".into(), script.into()]);
        let exit = Arc::new(Mutex::new(None));
        let seen = exit.clone();
        let spec = Spec { command, cwd: None, env: Vec::new() };
        let term = Terminal::spawn(spec, move |n| {
            if let Notice::Exited(code) = n {
                *seen.lock().unwrap() = Some(code);
            }
        })
        .expect("spawn the host PTY");
        Host { term, exit }
    }

    /// The host's scrollback and screen as text.
    pub fn screen(&self) -> String {
        self.term.text(false)
    }

    /// The screen's last row.
    pub fn last_row(&self) -> String {
        self.screen().lines().last().unwrap_or_default().trim_end().to_string()
    }

    /// Poll the host screen until `pred` holds (10 s), returning it.
    pub fn wait_for(&self, what: &str, pred: impl Fn(&str) -> bool) -> String {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let s = self.screen();
            if pred(&s) {
                return s;
            }
            if Instant::now() > end {
                panic!("timed out waiting for {what}; host screen was:\n{s}");
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    pub fn wait_text(&self, text: &str) -> String {
        self.wait_for(text, |s| s.contains(text))
    }

    /// Type bytes on the host keyboard.
    pub fn send(&self, bytes: &[u8]) {
        self.term.write(bytes).expect("write to the host PTY");
    }

    /// Wait until `pred` holds (10 s); for state outside the host screen.
    pub fn until(what: &str, pred: impl Fn() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while !pred() {
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    pub fn alt_screen(&self) -> bool {
        self.term.with_term(|t| t.mode().contains(TermMode::ALT_SCREEN))
    }

    pub fn cursor_shown(&self) -> bool {
        self.term.with_term(|t| t.mode().contains(TermMode::SHOW_CURSOR))
    }

    /// Bytes the script (and so `leon attach`) has written to the host so far.
    pub fn output_bytes(&self) -> u64 {
        self.term.output_bytes()
    }

    /// The pid of the shell running the script.
    pub fn pid(&self) -> i32 {
        self.term.pid()
    }

    pub fn exited(&self) -> Option<i32> {
        *self.exit.lock().unwrap()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.term.kill();
    }
}

/// `$LEON attach ARGS`, then the host's line discipline and a marker, so a test can check the
/// terminal came back cooked: prints `after-attach rc=N` and `stty: icanon echo` (or `-icanon`…).
pub fn attach_then_stty(args: &str) -> String {
    format!(
        "\"$LEON\" attach {args}; rc=$?; f=$(stty -a | tr ' ' '\\n' | grep -E '^-?(icanon|echo)$' | tr '\\n' ' '); \
         echo \"after-attach rc=$rc\"; echo \"stty: $f\"; exec sleep 300"
    )
}

/// The host was given back cooked: alt screen left, cursor shown, canonical mode and echo on.
pub fn assert_restored(h: &Host, rc: i32) {
    let s = h.wait_text("stty:");
    assert!(s.contains(&format!("after-attach rc={rc}")), "attach should exit {rc}:\n{s}");
    let flags = s.lines().find(|l| l.starts_with("stty:")).unwrap();
    assert!(flags.contains(" icanon") && flags.contains(" echo"), "raw mode must be off: {flags}");
    assert!(!flags.contains("-icanon") && !flags.contains("-echo "), "raw mode must be off: {flags}");
    assert!(!h.alt_screen(), "the alt screen must be left");
    assert!(h.cursor_shown(), "the cursor must be shown");
}

/// `words` appear in a row on screen, whatever the spacing (`od` pads its columns per platform).
pub fn has_words(screen: &str, words: &[&str]) -> bool {
    screen.split_whitespace().collect::<Vec<_>>().windows(words.len()).any(|w| w == words)
}
