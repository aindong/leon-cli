# Releases

Pushing a tag `vX.Y.Z` builds every release file, signs it, publishes a GitHub Release and updates
the Homebrew tap. This page is for whoever cuts a release, and for anyone writing code that reads
one: the manifest format below is a contract.

## What a release holds

Eight files, for version `V`:

- `Leon-V-macos-arm64.zip`, `Leon-V-macos-x86_64.zip`: Leon.app, signed ad hoc and not notarized.
- `leon-V-arm64-macos.tar.gz`, `leon-V-x86_64-macos.tar.gz`: the CLI alone for macOS.
- `leon-V-aarch64-linux.tar.gz`, `leon-V-x86_64-linux.tar.gz`: the CLI for Linux.
- `leon_V_arm64.deb`, `leon_V_amd64.deb`: the CLI as a Debian package.

Each tarball holds `leon`, `README.md`, `LICENSE`, `THIRD-PARTY-NOTICES.txt` and `docs/`. The CLI
ships as tarballs, not bare binaries, so that it never travels without its license notices.

Beside them: `SHA256SUMS`, `latest.json` (the manifest), and a `.minisig` signature for every one
of those ten files.

## Cutting a release

Before the first release, do the one-time setup under [Secrets](#secrets).

1. Set the version in `Cargo.toml` and give `CHANGELOG.md` a `## [X.Y.Z]` section. The release
   notes are that section's text.
2. Try it on your Mac: `scripts/release-dry-run.sh` builds the release (`scripts/release.sh`),
   signs it with a key made for the run, checks it, and then changes a byte to see the check fail.
   It needs Docker for the Linux packages; `--linux none` skips them.
3. Merge to `main` and wait for CI to pass on that commit. The release workflow builds and
   packages; it does not run the test suite again.
4. Tag that commit and push the tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.

`.github/workflows/release.yml` then runs:

- **version**: `scripts/release-version.sh` fails unless the tag is `v` plus the version in
  `Cargo.toml`. Each build also checks that `leon --version` prints that version.
- **macos**: `scripts/release.sh --linux none` on `macos-latest` (arm64) and `macos-15-intel`
  (x86_64). That runner is the last Intel image GitHub offers; when it goes, x86_64 macOS needs a
  cross-compile.
- **linux**: `scripts/package-linux.sh` on `ubuntu-24.04` and `ubuntu-24.04-arm`.
- **publish**: `scripts/release-manifest.sh --complete` signs everything and writes the manifest;
  `scripts/release-verify.sh --complete` checks the result against the keys leon pins; the
  release is created as a draft with `scripts/release-notes.sh`'s text, and only then published,
  so the latest release never has files missing.
- **homebrew**: `scripts/release-homebrew.sh` rewrites the tap's cask and formula.

A version with a `-` in it (`0.2.0-rc.1`) becomes a pre-release and leaves the tap alone.

### A test run

Start the workflow by hand (`workflow_dispatch`: the Actions tab, or `gh workflow run release.yml
--ref BRANCH`). It does everything except publish: the release stays a **draft**, no tag is
created, and the tap is not touched. Delete the draft afterwards (`gh release delete vX.Y.Z`);
publishing one by hand would create its tag and start a second run.

A test run signs with the real key, so its files are as trusted as a release's. Start one only
from a commit you would release.

## Secrets

Repository secrets, under Settings, Secrets and variables, Actions:

- **`LEON_RELEASE_KEY`** (required): the minisign secret key. Run `scripts/release-keys.sh` once,
  yourself. It makes the key pair in a private temp dir, hands the secret key to `gh secret set`
  from the file, deletes the temp dir, and adds the public key to `crates/leon/release-keys.pub`.
  Commit that file. The secret key is never printed and exists nowhere else afterwards; the
  script refuses to replace a secret that already exists.
- **`HOMEBREW_TAP_TOKEN`** (optional): lets the workflow push to the tap. Without it the tap job
  prints a notice and does nothing. To make it:
  1. GitHub, Settings, Developer settings, Personal access tokens, **Fine-grained** tokens,
     Generate new token.
  2. Resource owner: `aindong`. Repository access: Only select repositories,
     `aindong/homebrew-leon`.
  3. Repository permissions: Contents, **Read and write**. Nothing else.
  4. `gh secret set HOMEBREW_TAP_TOKEN --repo aindong/leon-cli`, and paste the token.

  Fine-grained tokens expire. With an expired token the tap job fails; make a new one and re-run
  that job.
- **`APPLE_DEVELOPER_ID`**: not used yet. See [Developer ID](#developer-id).

Anyone who can push to the repository can start the workflow, and so can sign with the key.

## Signatures

Every file is signed with [minisign](https://jedisct1.github.io/minisign/) (ed25519). HTTPS alone
only proves which server answered; the signature proves the file came from this workflow.

- The public keys leon trusts are the lines of `crates/leon/release-keys.pub`. A file is good if
  its `.minisig` verifies against any one of them.
- The workflow checks its own signatures against that file before it creates the release, so a
  secret that doesn't match the pinned key stops the run instead of shipping.
- Why minisign: the updater has to check the CLI on Linux as well as the app on macOS, so one
  verifier has to work everywhere. The `minisign-verify` crate (no dependencies) verifies these
  signatures in Rust: a key made by minisign 0.12 signed with 0.11 and verified with both the tool
  and the crate, and each rejected a changed byte. The alternative considered, the EdDSA appcast
  of a macOS app-update framework, covers only the app and needs that framework inside it.

`scripts/release-files.sh` is sourced by the scripts above: the list of the eight file names and
where releases are published.

## The manifest

`latest.json` describes one release. Its address never changes:

    https://github.com/aindong/leon-cli/releases/latest/download/latest.json

and its signature is at the same address with `.minisig` added.

| Field | Type | Meaning |
|---|---|---|
| `schema` | number | `1`. See the promise below. |
| `version` | string | The release's version, as in `Cargo.toml`: `0.1.0`. |
| `tag` | string | The git tag: `v` plus the version. |
| `commit` | string | The commit it was built from, 12 hex digits, as `leon --version` prints it. |
| `date` | string | When it was signed, UTC: `2026-11-15T12:59:18Z`. |
| `notes_url` | string | The release's page, with its notes. |
| `min_os` | object | The oldest systems the files run on. A key is absent when the release has no file for it. |
| `min_os.macos` | string | The lowest macOS version: `11.0`. |
| `min_os.linux_glibc` | string | The lowest glibc version: `2.34`. |
| `files` | array | One entry per release file. |
| `files[].name` | string | The file's name. |
| `files[].kind` | string | `app` (the zip of Leon.app), `cli` (a tarball) or `deb`. |
| `files[].os` | string | `macos` or `linux`. |
| `files[].arch` | string | `arm64` or `x86_64`, whatever the file's own name calls it. |
| `files[].url` | string | Where to download it. |
| `files[].size` | number | Its size in bytes. |
| `files[].sha256` | string | Its SHA-256, 64 hex digits. |
| `files[].signature` | string | The whole text of its `.minisig` file. |

A reader picks the entry whose `kind`, `os` and `arch` match.

**The promise.** Within schema `1`, fields are only ever added: ignore the ones you don't know.
Nothing documented here is removed, renamed or given another meaning. If that ever has to
happen, the new format gets a new file name and `latest.json` stays as it is, because installed
leons keep reading it.

**For an updater.** Verify `latest.json` against the pinned keys before reading it. Refuse a
`version` that isn't newer than the running one: an old release is still validly signed. Verify
the downloaded file's signature, not only its sha256.

## Checking a download

With the repository: put a release's files in a directory and run `scripts/release-verify.sh
--complete DIR`. It checks the sums, the manifest and every signature against the pinned keys.

Without it, one file at a time, with a key from `crates/leon/release-keys.pub`:

    minisign -Vm Leon-0.1.0-macos-arm64.zip -P <public key>

## Key rotation

leon trusts every key in `crates/leon/release-keys.pub`, so a new key can be introduced before
the old one stops signing. An installed leon only knows the keys it shipped with.

1. `scripts/release-keys.sh --name LEON_RELEASE_KEY_2` makes a second key under a second secret
   and pins its public key beside the first. Commit the file.
2. Cut a release. It is still signed with the old key, and the leon in it trusts both.
3. Once people have had time to update, change `secrets.LEON_RELEASE_KEY` in
   `.github/workflows/release.yml` to `secrets.LEON_RELEASE_KEY_2`. Releases are now signed with
   the new key.
4. Later, remove the old line from the keys file and delete the old secret.

A leon older than step 2 cannot verify a release from step 3 on. It updates through Homebrew or
a download from the Releases page.

If the key leaks, do the same steps without waiting. Nothing can make installed leons stop
trusting the leaked key; they have to update by hand to a release that no longer pins it.

## Homebrew

The tap is `aindong/homebrew-leon`:

    brew install --cask aindong/leon/leon   # Leon.app, with the leon command
    brew install aindong/leon/leon          # the CLI alone, macOS and Linux

The cask links the `leon` inside the app, so the two are not meant to be installed together.
Homebrew loads a third-party tap only when it is trusted; installing by the full name, as above,
trusts that one item ([Tap Trust](https://docs.brew.sh/Tap-Trust)).

**Quarantine.** Homebrew marks every cask download as quarantined, and macOS will not open an
app that is quarantined and not notarized. Homebrew removed its `--no-quarantine` flag, and its
policy for official casks says a cask "must not require System Integrity Protection or
Gatekeeper to be disabled or bypassed" (<https://docs.brew.sh/Acceptable-Casks>). That policy
binds Homebrew's own tap, not this one, but a cask that strips the flag in a hook does what
Homebrew chose to stop doing. So the cask leaves the flag alone and prints the command for you
to run:

    xattr -dr com.apple.quarantine /Applications/Leon.app

Run it before the first launch. A quarantined `leon` started from a shell does not fail: it
waits behind a system dialog. The formula is not affected; only casks are quarantined.

The workflow rewrites `Casks/leon.rb` and `Formula/leon.rb` from the manifest and pushes them.
To check them by hand: `brew style` and `brew audit --strict` on a local checkout of the tap.

## Developer ID

leon has no Apple Developer ID, so Leon.app is signed ad hoc and not notarized. That is why a
download needs the `xattr` command above.

The workflow has a step where Developer ID signing and notarization will go. It is not built.
Without the `APPLE_DEVELOPER_ID` secret it prints a notice and the release goes on; with the
secret set it fails, so nobody takes an ad-hoc build for a notarized one. Building it means
signing inside `scripts/release.sh`, between the bundle and the zip.
