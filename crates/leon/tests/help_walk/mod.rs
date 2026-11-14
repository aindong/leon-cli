//! Walks `leon … --help` for every listed command, so tests can check the help text and the docs
//! against what the binary really prints.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

/// One line of a help section: `spec` is the left column (`--json`, `[LABEL]`, `ls`), `desc` the rest.
pub struct Entry {
    pub section: String,
    pub spec: String,
    pub desc: String,
}

pub struct Help {
    /// The words after `leon`: empty for `leon` itself.
    pub path: Vec<String>,
    pub text: String,
    pub entries: Vec<Entry>,
}

impl Help {
    pub fn subcommands(&self) -> Vec<String> {
        self.section("Commands").map(|e| e.spec.clone()).filter(|n| n != "help").collect()
    }

    pub fn section<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Entry> + 'a {
        self.entries.iter().filter(move |e| e.section == name)
    }

    /// Every `--long` flag in the Options section.
    pub fn long_flags(&self) -> Vec<String> {
        self.section("Options")
            .filter_map(|e| e.spec.split([',', ' ', '[', '=']).find(|w| w.starts_with("--")).map(str::to_string))
            .collect()
    }
}

pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn help(path: &[String]) -> Help {
    let out = Command::new(env!("CARGO_BIN_EXE_leon")).args(path).arg("--help").output().unwrap();
    assert!(out.status.success(), "leon {path:?} --help failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    let mut entries = vec![];
    let mut section = String::new();
    for line in text.lines() {
        if let Some(name) = line.strip_suffix(':').filter(|n| !n.starts_with(' ') && !n.contains(' ')) {
            section = name.to_string();
        } else if line.starts_with("  ") && ["Commands", "Arguments", "Options"].contains(&section.as_str()) {
            // clap separates the left column from the description with two or more spaces
            let t = line.trim();
            let (spec, desc) = t.split_once("  ").unwrap_or((t, ""));
            entries.push(Entry {
                section: section.clone(),
                spec: spec.trim().to_string(),
                desc: desc.trim().to_string(),
            });
        } else if line.is_empty() || !line.starts_with(' ') {
            section.clear();
        }
    }
    Help { path: path.to_vec(), text, entries }
}

/// `leon --help` and the help of every command it lists, recursively.
pub fn all() -> Vec<Help> {
    let mut out = vec![];
    let mut todo = vec![vec![]];
    while let Some(path) = todo.pop() {
        let h = help(&path);
        for sub in h.subcommands() {
            let mut p = path.clone();
            p.push(sub);
            todo.push(p);
        }
        out.push(h);
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}
