# Writing a leon client

leon's server answers on a Unix socket, one JSON object per line. Any language that can open a
Unix socket and read and write JSON can drive it: create sessions, run programs and wait for them,
read their screens, and follow what changes. This guide covers what such a client needs, ends
with the compatibility promise, and shows a complete client in Python that a test runs against a
real server.

From a shell you don't need a client: `leon … --json` and `leon api call` cover the same ground
([cli.md](cli.md)). Every method is listed in [api.md](api.md), generated from the server itself.

```sh
leon api list
leon api describe session.create
leon api call server.version
```

## Find the socket

The socket is `server.sock` in leon's runtime directory, the first of:

1. `$LEON_RUNTIME_DIR`
2. `$XDG_RUNTIME_DIR/leon`
3. `~/.local/state/leon/run`

The directory is private (0700) and the socket 0600. The server only answers connections from
its own user. Programs that run inside leon also get `LEON_SERVER` (the socket path),
`LEON_SESSION` and `LEON_BLOCK` (their own session and block IDs).

Nothing starts the server for you on the socket. Run `leon server start` first (it returns once
the server is up, and does nothing if one already runs), or call any `leon` command, which
starts one on demand.

## Framing: NDJSON

Every message is one JSON object on one line, ended by `\n` (NDJSON). Write a request line, read
lines until the one with your `id` comes back. A request line can be at most 1 MiB; a longer one
gets an error and the connection closes.

A connection answers one call at a time, in order: you may write the next request before the
last reply arrives, but the server reads it only after that reply is out. Events and screens keep
arriving meanwhile. So a long call such as `block.wait` holds up its connection's later calls; give
it a connection of its own when you need to do other things while it waits.

## Requests, responses and errors

A request names a method and its `params` (an object; leaving it out means `{}`). The `id` is
yours: any JSON value, echoed back.

```json
{"id":1,"method":"session.create","params":{"label":"demo"}}
```

A success carries `result`:

```json
{"id":1,"result":{"session_id":"session:2574ef03-c286-4806-836f-d9607083c48b","window_id":"window:2b106ae7-0405-4a59-bcd6-75542f7c987c","block_id":"block:aed8252f-e1ce-4675-a481-141264fb3202"}}
```

A failure carries `"error"` with an HTTP-like `status`, a `code` to match on, and a `message`
for people:

```json
{"id":2,"error":{"status":404,"code":"session_not_found","message":"no session session:nope"}}
{"id":3,"error":{"status":400,"code":"bad_request","message":"session.create: unknown field `bogus`, expected one of `label`, `window_label`, `layout`"}}
{"id":4,"error":{"status":404,"code":"method_not_found","message":"no method nope"}}
```

| Code | Status | Means |
|---|---|---|
| `bad_request` | 400 | The params don't fit the method's input schema (unknown fields are refused), or a value is out of range. |
| `not_found`, `session_not_found` | 404 | No such block, window or session. |
| `method_not_found` | 404 | No such method (or block method). |
| `timeout` | 408 | A wait ran past its `timeout_ms` (504 when a config action ran out of time). |
| `ambiguous`, `conflict` | 409 | A query matches more than one target; or the state forbids it (`server.stop` with `if_idle` while sessions exist). |
| `resume_failed` | 410 | The event stream to resume is gone; attach again. |
| `request_too_large` | 413 | A request line over 1 MiB; the connection closes. |
| `io` | 500 | Writing to a terminal or similar failed. |
| `shutting_down` | 503 | The server is stopping. |

The table isn't complete (config actions add `config_error`, `action_failed` and `unavailable`),
and new codes may come in a minor release. Match on `code`, never on `message`, whose words may
change, and treat a code you don't know by its `status`. A line the server can't read as a
request gets an error with `"id": null`.

IDs are opaque strings such as `session:…`, `window:…` and `block:…`. Keep them; they stay the
same across server restarts. To turn something a person typed (a label, a position, an ID suffix)
into an ID, call `target.resolve`.

## A complete client

This client lists the sessions, runs a command in a new session, waits for it to exit and prints
its exit status. It uses only Python's standard library; `crates/leon/tests/m18_integrators.rs`
runs this exact block against a real server.

```python
#!/usr/bin/env python3
"""A leon client in plain Python: list sessions, run a command, wait for it."""
import json
import os
import socket
import sys


def socket_path():
    # The order leon uses: LEON_RUNTIME_DIR, XDG_RUNTIME_DIR/leon, ~/.local/state/leon/run.
    if os.environ.get("LEON_RUNTIME_DIR"):
        return os.path.join(os.environ["LEON_RUNTIME_DIR"], "server.sock")
    if os.environ.get("XDG_RUNTIME_DIR"):
        return os.path.join(os.environ["XDG_RUNTIME_DIR"], "leon", "server.sock")
    return os.path.expanduser("~/.local/state/leon/run/server.sock")


class Leon:
    def __init__(self, path):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.connect(path)
        self.file = self.sock.makefile("rwb")
        self.last_id = 0

    def call(self, method, **params):
        self.last_id += 1
        request = {"id": self.last_id, "method": method, "params": params}
        self.file.write(json.dumps(request).encode() + b"\n")
        self.file.flush()
        while True:
            line = self.file.readline()
            if not line:
                raise RuntimeError(f"{method}: the server closed the connection")
            reply = json.loads(line)
            if reply.get("id") == self.last_id:
                break  # other lines are events or screens, if this connection asked for them
        if "error" in reply:
            e = reply["error"]
            raise RuntimeError(f"{method}: {e['code']} ({e['status']}): {e['message']}")
        return reply["result"]


leon = Leon(socket_path())
api = leon.call("server.version")["api_version"]
if not api.startswith("1."):
    sys.exit(f"this client was written for api_version 1.x; the server speaks {api}")
print("api_version", api)

for s in leon.call("session.list")["sessions"]:
    print("session", s["session_id"], s.get("label") or "-", s["windows"], "windows")

# Keep the block open when its program exits, so the session is still there to destroy.
options = {"command": ["sh", "-c", "echo hello from leon; exit 3"], "exit": {"on_completion": False}}
made = leon.call("session.create", label="python-demo", layout={"block": {"options": options}})
done = leon.call("block.wait", block_id=made["block_id"], timeout_ms=10000)
print("exit", done["exit_code"])
leon.call("session.destroy", session_id=made["session_id"])
```

To type into a program, read its screen as text, or ask its status, call `block.method` with a
block method and its `args`:

```json
{"id":5,"method":"block.method","params":{"session_id":"session:…","block_id":"block:…","method":"dev.leon.terminal.write","args":{"data":"ls\r"}}}
{"id":6,"method":"block.method","params":{"session_id":"session:…","block_id":"block:…","method":"dev.leon.terminal.format","args":{"trim":true}}}
```

`dev.leon.terminal.program_status` returns the program's OSC 7501 status records, and
`block.event` sends a key (`{"type":"key","key":"ctrl+c"}`) or a paste the way a person would.

## Events: subscribe, follow `seq`, resume

`session.attach_all {event_scope}` subscribes the connection to session lifecycle events
(`session`) or every event (`all`); `session.attach {session_id}` to one session's events, block
events included. The reply is a snapshot with the connection's current `"seq"`; after it, events
arrive on the same connection as lines of their own, each with the next `seq`:

```json
{"id":4,"result":{"seq":0,"sessions":[],"resume":"resume:4902d197-515f-49b8-a0c4-090e7231d443"}}
{"event":{"type":"session_created","payload":{"session":{"session_id":"session:2574ef03-c286-4806-836f-d9607083c48b","label":"demo"}}},"seq":1}
{"event":{"type":"session_event","payload":{"session_id":"session:2574ef03-c286-4806-836f-d9607083c48b","event":{"type":"block_created","payload":{"block_id":"block:aed8252f-e1ce-4675-a481-141264fb3202"}}}},"seq":2}
{"event":{"type":"session_event","payload":{"session_id":"session:2574ef03-c286-4806-836f-d9607083c48b","event":{"type":"session_view_changed","payload":{"revision":1}}}},"seq":3}
```

Apply the snapshot, then every event with a higher `seq`. `seq` is per connection and has no
gaps, so a jump means you missed something. Events and replies share the connection, so read
every line and sort them by whether they carry `id` or `event`. A connection that falls more than
8 MiB behind is dropped.

With `"resumable": true` the reply also carries a `resume` token. If the connection drops, open a
new one and call `session.resume {resume, seq}` with the last `seq` you applied: the events after
it come again, then the new ones, with the same counter. The stream waits 60 s for you. If
`session.resume` fails with `resume_failed` (the server restarted, or too much happened), attach
again and start from the new snapshot. `leon events -H` does exactly this.

Rather than applying events one by one, a client can refetch what it shows
(`session.list`, `session.view`, `session.list_blocks`) after each batch; the app does that.

## Screens (experimental)

`block.watch {block_ids}` on a connection makes the server push each watched block's screen as
`{"screen":{block_id, full, lines, cols, rows, cursor, app_cursor, bracketed_paste}}` lines: a
whole screen first (`full`), then only the rows that changed. Screen lines have no `seq`: each is
a diff against the last one sent for that block, and a slow reader gets fewer, newer screens
rather than a backlog. Calling it again replaces the set; `[]` stops. Scrollback comes in ranges
from `dev.leon.terminal.lines`. [architecture.md](architecture.md) describes the format in full.
These methods are `experimental`: they follow what the app and `leon attach` need, and may change
in any release.

## Over SSH: `leon server proxy`

On another machine, `leon server proxy` connects its stdin and stdout to that machine's server
(starting a persistent one if none runs). So a client reaches a remote leon by running ssh and
speaking the same NDJSON over its pipes, with nothing listening on TCP:

```text
ssh -T me@devbox leon server proxy
```

Spawn that command, write request lines to its stdin and read replies and events from its stdout.
ssh runs the proxy as your user on the host, which is what the server's same-user check needs.
`leon -H` does the same with a few safety options (no forwarding, keepalives), described in
[cli/hosts.md](cli/hosts.md). When the link drops, resume the event stream as above.

## Compatibility

The socket API has a version of its own, `api_version`, reported by `help.all` and
`server.version` (it is `1.8.0` today; [CHANGELOG.md](../CHANGELOG.md) lists every change). It
follows semver, for the methods marked `stable`:

- A **major** bump (2.0.0) may break clients: remove or rename a stable method or field, change a
  type or a meaning, add a required parameter. Each one is listed in the changelog.
- A **minor** bump (1.1.0) only adds: new methods, new **optional** parameters, new result fields,
  new event types and fields, new error codes, new block methods. Until the next major version, a
  stable method only gains optional fields.
- A **patch** bump (1.0.1) changes no schema.

So a client written for 1.x should check that the major version is 1, ignore fields and event
types it doesn't know, and treat an unknown error `code` by its `status`.

**Stability.** Every method carries one of:

- `stable`: covered by the rules above.
- `experimental`: may change or go away in any release, minor and patch included. Today that is
  the screen transport (`block.watch` and the `dev.leon.terminal` screen, `lines`, `find`,
  `selection` and `link_at` methods) and `client.action` and `client.key`.
- `internal`: for leon's own clients (`client.keymap`); don't build on it.

`leon api list` shows each method's stability, and [api.md](api.md) marks it on every method.

**Deprecation.** A stable method is never removed without warning. First it is marked
`deprecated`, with the version that deprecated it (`since`), the method to call instead
(`replacement`), and the major version that removes it (`removal`):

```json
{"name":"x.old","stability":"stable","deprecated":{"since":"1.2.0","replacement":"x.new","removal":"2.0.0"}}
```

It keeps working until that removal version, and at least one minor release separates the
deprecation from the removal. `leon api describe` prints the replacement. No method is deprecated
today.

**How it is held.** [api-snapshot.json](api-snapshot.json) keeps the input and output schemas of
every stable method, block methods included. A test compares the server's schemas with it and
fails on any difference; the snapshot can only be updated together with a higher `API_VERSION`
(`crates/leon-server/src/contract.rs`): a major one when a stable method is removed or stops
being stable, at least a minor one for any other change. The test can't tell an added optional
field from a breaking edit inside a method, so that choice stays with the author and reviewer. Event bodies have no schema in the registry, so the event
names and fields shown in this guide are covered by the policy but not checked by that test.

**Not covered:** the wording of descriptions and error messages, the order of keys in an object,
and the CLI's text output. The CLI's commands, flags and exit codes are documented in
[cli.md](cli.md), whose own test keeps them in step with `--help`.
