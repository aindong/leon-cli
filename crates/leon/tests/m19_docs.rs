//! M19 criteria 5 and 12 (.agent/plans/2026-11-15-m19-tmux-migrants.md): the docs cover repeat
//! keys, shifted symbols, the tmux preset, the vim-navigator recipe, copy mode and hints; the
//! palette and `leon actions` list the new actions. (docs/lua.md's `lua` blocks themselves are run
//! by m7b_lua_doc.)

mod help_walk;
mod support;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap()
}

/// The fenced `lua` blocks of docs/lua.md.
fn lua_blocks() -> Vec<String> {
    let lua = doc("docs/lua.md");
    let blocks: Vec<&str> = lua.split("```").collect();
    blocks.iter().skip(1).step_by(2).filter_map(|b| b.strip_prefix("lua\n")).map(String::from).collect()
}

#[test]
fn lua_md_has_the_preset_and_the_recipe() {
    let lua = doc("docs/lua.md");
    for word in ["repeats = true", "repeat_time", "hint_patterns", "shift+", "client.copy_mode", "client.hints"] {
        assert!(lua.contains(word), "docs/lua.md documents {word}");
    }
    let preset = lua_blocks().into_iter().find(|b| b.contains("tmux")).expect("a tmux-style preset");
    for key in [
        "ctrl+b>%",
        "ctrl+b>\\\"",
        "ctrl+b>h",
        "ctrl+b>j",
        "ctrl+b>k",
        "ctrl+b>l",
        "ctrl+b>z",
        "ctrl+b>c",
        "ctrl+b>n",
        "ctrl+b>p",
        "ctrl+b>x",
        "ctrl+b>[",
        "ctrl+b>shift+f",
        "repeats = true",
    ] {
        assert!(preset.contains(key), "the preset binds {key}:\n{preset}");
    }
    let recipe = lua_blocks().into_iter().find(|b| b.contains("nvim")).expect("the vim-navigator recipe");
    assert!(recipe.contains("ctrl+h") || recipe.contains("\"ctrl+\" .."), "{recipe}");
    assert!(lua.contains("leon focus") && lua.contains("LEON_BLOCK"), "vim's side of the recipe");
}

#[test]
fn readme_architecture_and_changelog() {
    let readme = doc("README.md");
    for key in ["⌘⇧C", "⌘⇧H"] {
        assert!(readme.contains(key), "README lists {key}");
    }
    let arch =
        ["docs/architecture.md", "docs/architecture/protocol-and-clients.md", "docs/architecture/state-and-hosts.md"]
            .map(doc)
            .concat();
    for topic in ["Copy mode", "Hints", "repeat_time"] {
        assert!(arch.contains(topic), "architecture covers {topic}");
    }
    assert!(arch.contains("no copy mode"), "architecture says why leon attach has no copy mode");
    assert!(doc("CHANGELOG.md").contains("M19"));
    assert!(doc("docs/cli.md").contains("the app's own actions"), "docs/cli.md says what leon actions lists");
}

#[test]
fn the_palette_and_leon_actions_list_the_new_actions() {
    for name in ["client.copy_mode", "client.hints"] {
        assert!(leon_app::palette::BUILTINS.iter().any(|(n, _)| *n == name), "the palette offers {name}");
    }
    let env = support::Env::new();
    let text = env.ok(&["actions"]);
    let json = env.json(&["actions", "--json"]);
    for name in ["client.copy_mode", "client.hints", "pane.split.right"] {
        assert!(text.lines().any(|l| l.starts_with(name)), "leon actions lists {name}:\n{text}");
        let row = json.as_array().unwrap().iter().find(|a| a["name"] == name);
        assert_eq!(row.map(|r| r["source"].clone()), Some(serde_json::json!("app")), "--json has {name}");
    }
}
