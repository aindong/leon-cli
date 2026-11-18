## What and why

<!-- What this changes, and the problem it solves. Link the issue: "Fixes #123". -->

## How it was tested

<!-- The tests you added or ran, and anything you checked by hand (the app, a terminal, Linux). -->

## Checklist

- [ ] Branched from `dev`, and this pull request goes into `dev`
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and the tests that cover the change pass
- [ ] Docs updated where behavior changed (`docs/cli.md` for a command or flag, `docs/shortcuts.md` for an app shortcut)
- [ ] A new feature has its bullet in `CHANGELOG.md`'s `[Unreleased]`; an API change regenerated `docs/api.md`
- [ ] Nothing private in code, docs, commits or screenshots (see CONTRIBUTING.md)
- [ ] Translations only: `scripts/i18n-check.sh` passes
