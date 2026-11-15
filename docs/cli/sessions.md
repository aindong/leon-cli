# CLI: sessions and windows

Part of the [command reference](../cli.md): creating, listing, renaming and ending sessions and
their windows (tabs).

## Sessions

### `leon new [LABEL] [--cwd DIR]`

Create a session and print its ID. Same as `leon session new`.

- `-c, --cwd DIR`: start the first terminal in this existing directory. Relative paths resolve
  against the caller's working directory. A missing path or a file exits 2 (usage error).
  Omitted uses the server's home directory. With `-H` the path is the host's: it must be
  absolute, and the host checks it.

### `leon ls`

List sessions. Same as `leon session ls`.

### `leon kill SESSION…`

Destroy sessions and end every program in them. Same as `leon session kill`.

### `leon session`

Manage sessions.

### `leon session new [LABEL] [--cwd DIR]`

Create a session and print its ID.

### `leon session ls`

List sessions: ID, label, window count.

### `leon session inspect [SESSION]`

Show a session and its windows (default: `-s`, else `$LEON_SESSION`).

### `leon session rename SESSION LABEL`

Rename a session.

### `leon session kill SESSION…`

Destroy sessions.

```sh
leon new work
leon session rename work "my project"
leon ls --json
```

## Windows (tabs)

### `leon window`

Manage the windows of a session (`-s`).

### `leon window new [LABEL]`

Add a window running a login shell and print its ID.

### `leon window ls`

List windows in tab order.

### `leon window rename WINDOW LABEL`

Rename a window. `WINDOW` is an ID, label, 1-based tab position or ID suffix.

### `leon window close WINDOW`

Close a window and end its programs. Closing a session's last window destroys the session.
