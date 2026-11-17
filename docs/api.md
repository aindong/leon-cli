# leon API reference

<!-- Generated from `help.all` by crates/leon-server/tests/m18_api_docs.rs; don't edit by hand.
     Change the registry, then run `LEON_BLESS=1 cargo test -p leon-server --test m18_api_docs`. -->

API version **1.9.0**. Every method is marked:

- *stable*: covered by the [compatibility policy](integrators.md#compatibility). It only gains
  optional fields until the next major version.
- *experimental*: may change or go away in any release.
- *internal*: for leon's own clients; don't build on it.

Call a method by writing one JSON line to the server's socket, such as
`{"id":1,"method":"session.list","params":{}}`; [integrators.md](integrators.md)
explains the socket, the framing, errors and events. `leon api describe METHOD` prints the
same facts for the server you run.

## Contents

- [help](#help): 1
- [server](#server): 6
- [session](#session): 28
- [target](#target): 1
- [block](#block): 5
- [client](#client): 3
- [Block methods: dev.leon.terminal](#block-methods-devleonterminal): 14

## help

### `help.all`

*stable*. Every method with its description and JSON Schemas.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `api_version` | string | yes | The API's semver (docs/integrators.md#compatibility), apart from the build's version. |
| `methods` | [`MethodDoc`] | yes |  |
| `block_types` | [`BlockType`] | yes | Block creators, their flavors, and the block methods `block.method` calls. |

**`MethodDoc`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes |  |
| `description` | string | yes |  |
| `stability` | `Stability` | yes | `stable` (covered by the compatibility policy), `experimental` or `internal`. |
| `deprecated` | `Deprecated` \| null |  | Present once the method is on its way out. |
| `input_schema` | any | yes |  |
| `output_schema` | any | yes |  |
| `request_example` | any |  | Example `params`, for the methods that have one. |
| `response_example` | any |  | The `result` that answers `request_example`. |

**`Stability`:** `"stable"` | `"experimental"` | `"internal"`.

**`Deprecated`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `since` | string | yes | The api_version that deprecated the method. |
| `replacement` | string | yes | The method to call instead. |
| `removal` | string | yes | The api_version that removes it (a major one). |

**`BlockType`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `creator` | string | yes |  |
| `flavors` | [string] | yes |  |
| `methods` | [`MethodDoc`] | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoParams",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Help",
    "type": "object",
    "properties": {
      "api_version": {
        "description": "The API's semver (docs/integrators.md#compatibility), apart from the build's version.",
        "type": "string"
      },
      "methods": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/MethodDoc"
        }
      },
      "block_types": {
        "description": "Block creators, their flavors, and the block methods `block.method` calls.",
        "type": "array",
        "items": {
          "$ref": "#/$defs/BlockType"
        }
      }
    },
    "required": [
      "api_version",
      "methods",
      "block_types"
    ],
    "$defs": {
      "MethodDoc": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string"
          },
          "description": {
            "type": "string"
          },
          "stability": {
            "description": "`stable` (covered by the compatibility policy), `experimental` or `internal`.",
            "$ref": "#/$defs/Stability"
          },
          "deprecated": {
            "description": "Present once the method is on its way out.",
            "anyOf": [
              {
                "$ref": "#/$defs/Deprecated"
              },
              {
                "type": "null"
              }
            ]
          },
          "input_schema": true,
          "output_schema": true,
          "request_example": {
            "description": "Example `params`, for the methods that have one."
          },
          "response_example": {
            "description": "The `result` that answers `request_example`."
          }
        },
        "required": [
          "name",
          "description",
          "stability",
          "input_schema",
          "output_schema"
        ]
      },
      "Stability": {
        "oneOf": [
          {
            "description": "Covered by the compatibility policy: until the next major version it only gains optional fields.",
            "type": "string",
            "const": "stable"
          },
          {
            "description": "May change or go away in any release.",
            "type": "string",
            "const": "experimental"
          },
          {
            "description": "For leon's own clients (the app, `leon attach`); don't build on it.",
            "type": "string",
            "const": "internal"
          }
        ]
      },
      "Deprecated": {
        "type": "object",
        "properties": {
          "since": {
            "description": "The api_version that deprecated the method.",
            "type": "string"
          },
          "replacement": {
            "description": "The method to call instead.",
            "type": "string"
          },
          "removal": {
            "description": "The api_version that removes it (a major one).",
            "type": "string"
          }
        },
        "required": [
          "since",
          "replacement",
          "removal"
        ]
      },
      "BlockType": {
        "type": "object",
        "properties": {
          "creator": {
            "type": "string"
          },
          "flavors": {
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "methods": {
            "type": "array",
            "items": {
              "$ref": "#/$defs/MethodDoc"
            }
          }
        },
        "required": [
          "creator",
          "flavors",
          "methods"
        ]
      }
    }
  }
}
```

</details>

## server

### `server.status`

*stable*. Whether the server runs, its pid, version and socket.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `state` | string | yes | Always `running` when the server answers. |
| `pid` | integer (uint32) | yes |  |
| `version` | string | yes |  |
| `socket` | string | yes |  |
| `autostarted` | boolean | yes | True when a client started this server on demand; it exits once idle. |
| `sessions` | integer (uint) | yes |  |
| `state_file` | string \| null |  | Where sessions are saved, while running and on stop; null when they aren't (the state dir isn't private). |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoParams",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Status",
    "type": "object",
    "properties": {
      "state": {
        "description": "Always `running` when the server answers.",
        "type": "string"
      },
      "pid": {
        "type": "integer",
        "format": "uint32",
        "minimum": 0
      },
      "version": {
        "type": "string"
      },
      "socket": {
        "type": "string"
      },
      "autostarted": {
        "description": "True when a client started this server on demand; it exits once idle.",
        "type": "boolean"
      },
      "sessions": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "state_file": {
        "description": "Where sessions are saved, while running and on stop; null when they aren't (the state dir isn't private).",
        "type": [
          "string",
          "null"
        ]
      }
    },
    "required": [
      "state",
      "pid",
      "version",
      "socket",
      "autostarted",
      "sessions"
    ]
  }
}
```

</details>

### `server.persist`

*stable*. Keep running when idle (an autostarted server otherwise exits).

**Input:** none

**Output:** none

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoParams",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Empty",
    "type": "object"
  }
}
```

</details>

### `server.version`

*stable*. The server's version.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `version` | string | yes | The build's version. |
| `api_version` | string | yes | The API's semver, as in `help.all`. |

**Example:**

```json
{"id":1,"method":"server.version","params":{}}
{"id":1,"result":{"version":"0.1.0","api_version":"1.9.0"}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoParams",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Version",
    "type": "object",
    "properties": {
      "version": {
        "description": "The build's version.",
        "type": "string"
      },
      "api_version": {
        "description": "The API's semver, as in `help.all`.",
        "type": "string"
      }
    },
    "required": [
      "version",
      "api_version"
    ]
  }
}
```

</details>

### `server.stop`

*stable*. Stop the server after replying; every program ends. Sessions are saved unless `discard`.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `if_idle` | boolean |  | Only stop when no session exists; otherwise fail with `conflict`. |
| `discard` | boolean |  | Don't save the sessions, and forget the ones saved before: the next start is empty. |

**Output:** none

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Stop",
    "type": "object",
    "properties": {
      "if_idle": {
        "description": "Only stop when no session exists; otherwise fail with `conflict`.",
        "type": "boolean",
        "default": false
      },
      "discard": {
        "description": "Don't save the sessions, and forget the ones saved before: the next start is empty.",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Empty",
    "type": "object"
  }
}
```

</details>

### `server.settings`

*stable*. The effective settings from init.lua (theme, font, notifications, copy_on_select, resume), defaults filled in, and where each was set. A reload that changes them emits `settings_changed` with the same object to `--all` subscribers.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `settings` | `Effective` | yes | The effective settings: init.lua's `leon.set` calls over the defaults. |
| `sources` | object | yes | Where each setting was last set, as `file:line`, by key (`font.size`); a setting at its default isn't listed. |

**`Effective`:** The shape of `leon_lua::Settings::to_json`, which builds it.

| Field | Type | Required | Description |
|---|---|---|---|
| `theme` | `ThemeChoice` | yes |  |
| `font` | `Font` | yes |  |
| `notifications` | `Notifications` | yes |  |
| `copy_on_select` | boolean | yes | The app copies a selection to the clipboard as it is made. |
| `repeat_time` | integer (uint64) | yes | Milliseconds after a repeatable binding during which its keys work without the prefix (0 turns repeat off). |
| `hint_patterns` | [string] | yes | Regular expressions hints match besides the built-in ones. |
| `resume` | string | yes | What a restored block does with its program's resume command: `ask`, `always` or `never`. |
| `fade_inactive_panes` | number | yes | How far the app fades the panes without the focus toward the window's background, from 0 (off) to 0.8. |
| `check_for_updates` | boolean | yes | Leon.app looks for a newer release once a day; false, only when asked. |
| `translucency` | number | yes | How much of the desktop shows through Leon.app's window, from 0 (opaque) to 0.8. |

**`ThemeChoice`:** A theme name for each OS appearance (the same one for both unless init.lua gives two).

| Field | Type | Required | Description |
|---|---|---|---|
| `dark` | string | yes |  |
| `light` | string | yes |  |

**`Font`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `family` | string | yes |  |
| `size` | number | yes | Points, from 8 to 32. |

**`Notifications`:** Which notifications the app posts.

| Field | Type | Required | Description |
|---|---|---|---|
| `done` | boolean | yes | A long run finished while the app was in the background. |
| `blocked` | boolean | yes | A block became blocked. |
| `error` | boolean | yes | A block reported an error. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoParams",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SettingsView",
    "type": "object",
    "properties": {
      "settings": {
        "description": "The effective settings: init.lua's `leon.set` calls over the defaults.",
        "$ref": "#/$defs/Effective"
      },
      "sources": {
        "description": "Where each setting was last set, as `file:line`, by key (`font.size`); a setting at its\ndefault isn't listed.",
        "type": "object",
        "additionalProperties": {
          "type": "string"
        }
      }
    },
    "required": [
      "settings",
      "sources"
    ],
    "$defs": {
      "Effective": {
        "description": "The shape of `leon_lua::Settings::to_json`, which builds it.",
        "type": "object",
        "properties": {
          "theme": {
            "$ref": "#/$defs/ThemeChoice"
          },
          "font": {
            "$ref": "#/$defs/Font"
          },
          "notifications": {
            "$ref": "#/$defs/Notifications"
          },
          "copy_on_select": {
            "description": "The app copies a selection to the clipboard as it is made.",
            "type": "boolean"
          },
          "repeat_time": {
            "description": "Milliseconds after a repeatable binding during which its keys work without the prefix\n(0 turns repeat off).",
            "type": "integer",
            "format": "uint64",
            "minimum": 0
          },
          "hint_patterns": {
            "description": "Regular expressions hints match besides the built-in ones.",
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "resume": {
            "description": "What a restored block does with its program's resume command: `ask`, `always` or `never`.",
            "type": "string"
          },
          "fade_inactive_panes": {
            "description": "How far the app fades the panes without the focus toward the window's background, from 0\n(off) to 0.8.",
            "type": "number",
            "format": "double"
          },
          "check_for_updates": {
            "description": "Leon.app looks for a newer release once a day; false, only when asked.",
            "type": "boolean"
          },
          "translucency": {
            "description": "How much of the desktop shows through Leon.app's window, from 0 (opaque) to 0.8.",
            "type": "number",
            "format": "double"
          }
        },
        "required": [
          "theme",
          "font",
          "notifications",
          "copy_on_select",
          "repeat_time",
          "hint_patterns",
          "resume",
          "fade_inactive_panes",
          "check_for_updates",
          "translucency"
        ]
      },
      "ThemeChoice": {
        "description": "A theme name for each OS appearance (the same one for both unless init.lua gives two).",
        "type": "object",
        "properties": {
          "dark": {
            "type": "string"
          },
          "light": {
            "type": "string"
          }
        },
        "required": [
          "dark",
          "light"
        ]
      },
      "Font": {
        "type": "object",
        "properties": {
          "family": {
            "type": "string"
          },
          "size": {
            "description": "Points, from 8 to 32.",
            "type": "number",
            "format": "double"
          }
        },
        "required": [
          "family",
          "size"
        ]
      },
      "Notifications": {
        "description": "Which notifications the app posts.",
        "type": "object",
        "properties": {
          "done": {
            "description": "A long run finished while the app was in the background.",
            "type": "boolean"
          },
          "blocked": {
            "description": "A block became blocked.",
            "type": "boolean"
          },
          "error": {
            "description": "A block reported an error.",
            "type": "boolean"
          }
        },
        "required": [
          "done",
          "blocked",
          "error"
        ]
      }
    }
  }
}
```

</details>

### `server.config_reload`

*stable*. Re-read init.lua. A failed load keeps the last good config; a good one bumps the keymap generation and emits `keymap_changed`, and `settings_changed` if a setting changed. Local socket only.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `generation` | integer (uint64) | yes |  |
| `path` | string | yes |  |
| `bindings` | integer (uint) | yes |  |
| `modes` | integer (uint) | yes |  |
| `actions` | integer (uint) | yes |  |
| `diagnostics` | [string] | yes | Skipped calls and warnings, as `file:line: level: message`. |
| `logs` | [string] | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoParams",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Reloaded",
    "type": "object",
    "properties": {
      "generation": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      },
      "path": {
        "type": "string"
      },
      "bindings": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "modes": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "actions": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "diagnostics": {
        "description": "Skipped calls and warnings, as `file:line: level: message`.",
        "type": "array",
        "items": {
          "type": "string"
        }
      },
      "logs": {
        "type": "array",
        "items": {
          "type": "string"
        }
      }
    },
    "required": [
      "generation",
      "path",
      "bindings",
      "modes",
      "actions",
      "diagnostics",
      "logs"
    ]
  }
}
```

</details>

## session

### `session.list`

*stable*. Every session in order, with its label and window count.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `sessions` | [`SessionSummary`] | yes |  |

**`SessionSummary`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `label` | string \| null |  |  |
| `windows` | integer (uint) | yes | Number of windows (tabs). |

**Example:**

```json
{"id":1,"method":"session.list","params":{}}
{"id":1,"result":{"sessions":[{"session_id":"session:5b1c2f0e-8d7a-4c39-9a61-2e4f7b0c9d13","label":"web","windows":2}]}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoParams",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SessionList",
    "type": "object",
    "properties": {
      "sessions": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/SessionSummary"
        }
      }
    },
    "required": [
      "sessions"
    ],
    "$defs": {
      "SessionSummary": {
        "type": "object",
        "properties": {
          "session_id": {
            "type": "string"
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "windows": {
            "description": "Number of windows (tabs).",
            "type": "integer",
            "format": "uint",
            "minimum": 0
          }
        },
        "required": [
          "session_id",
          "windows"
        ]
      }
    }
  }
}
```

</details>

### `session.create`

*stable*. Create a session with one window holding one terminal.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `label` | string \| null |  |  |
| `window_label` | string \| null |  | Label for the session's first window. |
| `cwd` | string \| null |  | Directory for the first terminal, overriding layout.block.options.cwd. Must exist; relative paths resolve against the server's cwd. Omitted preserves the layout default. |
| `layout` | `Layout` \| null |  | What the first window holds; default a login shell. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `window_id` | string | yes |  |
| `block_id` | string | yes |  |

**`Layout`:** { block: `BlockSpec` }. What a new window or split holds: one block (nested layouts aren't supported yet).

**`BlockSpec`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `flavor` | string \| null |  | Only `dev.leon.terminal.shell` exists (the default). |
| `label` | string \| null |  |  |
| `options` | `TerminalOptions` |  |  |

**`TerminalOptions`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `command` | [string] \| null |  | argv to run directly; omitted runs the login shell. |
| `cwd` | string \| null |  | Working directory; defaults to `$HOME`. |
| `exit` | `ExitOptions` \| null |  |  |

**`ExitOptions`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `on_completion` | boolean | yes | Close the block when its program exits (default true); false keeps it open. |

**Example:**

```json
{"id":1,"method":"session.create","params":{"label":"web","cwd":"/tmp","layout":{"block":{"options":{"command":["ls","-l"],"exit":{"on_completion":false}}}}}}
{"id":1,"result":{"session_id":"session:5b1c2f0e-8d7a-4c39-9a61-2e4f7b0c9d13","window_id":"window:0f3e9a42-7c15-4b8e-a2d6-91c4e5b7f028","block_id":"block:c7d2e816-3a9f-4f05-8b1e-6d0a4c2f9b57"}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "CreateSession",
    "type": "object",
    "properties": {
      "label": {
        "type": [
          "string",
          "null"
        ]
      },
      "window_label": {
        "description": "Label for the session's first window.",
        "type": [
          "string",
          "null"
        ]
      },
      "cwd": {
        "description": "Directory for the first terminal, overriding layout.block.options.cwd. Must exist;\nrelative paths resolve against the server's cwd. Omitted preserves the layout default.",
        "type": [
          "string",
          "null"
        ]
      },
      "layout": {
        "description": "What the first window holds; default a login shell.",
        "anyOf": [
          {
            "$ref": "#/$defs/Layout"
          },
          {
            "type": "null"
          }
        ]
      }
    },
    "additionalProperties": false,
    "$defs": {
      "Layout": {
        "description": "What a new window or split holds: one block (nested layouts aren't supported yet).",
        "oneOf": [
          {
            "type": "object",
            "properties": {
              "block": {
                "$ref": "#/$defs/BlockSpec"
              }
            },
            "required": [
              "block"
            ],
            "additionalProperties": false
          }
        ]
      },
      "BlockSpec": {
        "type": "object",
        "properties": {
          "flavor": {
            "description": "Only `dev.leon.terminal.shell` exists (the default).",
            "type": [
              "string",
              "null"
            ]
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "options": {
            "$ref": "#/$defs/TerminalOptions"
          }
        },
        "additionalProperties": false
      },
      "TerminalOptions": {
        "type": "object",
        "properties": {
          "command": {
            "description": "argv to run directly; omitted runs the login shell.",
            "type": [
              "array",
              "null"
            ],
            "items": {
              "type": "string"
            }
          },
          "cwd": {
            "description": "Working directory; defaults to `$HOME`.",
            "type": [
              "string",
              "null"
            ]
          },
          "exit": {
            "anyOf": [
              {
                "$ref": "#/$defs/ExitOptions"
              },
              {
                "type": "null"
              }
            ]
          }
        },
        "additionalProperties": false
      },
      "ExitOptions": {
        "type": "object",
        "properties": {
          "on_completion": {
            "description": "Close the block when its program exits (default true); false keeps it open.",
            "type": "boolean"
          }
        },
        "additionalProperties": false,
        "required": [
          "on_completion"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Created",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "window_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      }
    },
    "required": [
      "session_id",
      "window_id",
      "block_id"
    ]
  }
}
```

</details>

### `session.destroy`

*stable*. Destroy a session and everything in it.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |

**Output:** none

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SessionRef",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Empty",
    "type": "object"
  }
}
```

</details>

### `session.set_label`

*stable*. Rename a session.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `label` | string | yes | The new label; empty clears it. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SetLabel",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "label": {
        "description": "The new label; empty clears it.",
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "label"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.view`

*stable*. A session's windows in tab order, each with its layout tree, focused and zoomed block.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `label` | string \| null |  |  |
| `revision` | integer (uint64) | yes | Bumped by every change to this view. |
| `windows` | [`WindowView`] | yes |  |

**`WindowView`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `window_id` | string | yes |  |
| `label` | string \| null |  |  |
| `position` | integer (uint) | yes | 1-based position in tab order. |
| `active` | boolean | yes |  |
| `block_ids` | [string] | yes | In layout order. |
| `layout` | `Node` \| null |  |  |
| `floating` | [`Floating`] | yes | Back to front, including hidden floats. |
| `docked` | [`Docked`] | yes | At most one panel per side, including hidden drawers. |
| `focused_block_id` | string \| null |  |  |
| `zoomed_block_id` | string \| null |  | The block shown filling the window; the layout is unchanged. |

**`Node`:** { block: { block_id: string } } | { split: { split_id: string, direction: `Direction`, ratio: number, before: `Node`, after: `Node` } }.

**`Direction`:** `"horizontal"` | `"vertical"`.

**`Floating`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `bounds` | `Bounds` | yes |  |
| `hidden` | boolean | yes |  |

**`Bounds`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `x` | number | yes |  |
| `y` | number | yes |  |
| `width` | number | yes |  |
| `height` | number | yes |  |

**`Docked`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `side` | `DockSide` | yes |  |
| `size` | number | yes |  |
| `hidden` | boolean | yes |  |

**`DockSide`:** `"left"` | `"right"` | `"bottom"`.

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SessionRef",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SessionView",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "label": {
        "type": [
          "string",
          "null"
        ]
      },
      "revision": {
        "description": "Bumped by every change to this view.",
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      },
      "windows": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/WindowView"
        }
      }
    },
    "required": [
      "session_id",
      "revision",
      "windows"
    ],
    "$defs": {
      "WindowView": {
        "type": "object",
        "properties": {
          "window_id": {
            "type": "string"
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "position": {
            "description": "1-based position in tab order.",
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "active": {
            "type": "boolean"
          },
          "block_ids": {
            "description": "In layout order.",
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "layout": {
            "anyOf": [
              {
                "$ref": "#/$defs/Node"
              },
              {
                "type": "null"
              }
            ]
          },
          "floating": {
            "description": "Back to front, including hidden floats.",
            "type": "array",
            "items": {
              "$ref": "#/$defs/Floating"
            }
          },
          "docked": {
            "description": "At most one panel per side, including hidden drawers.",
            "type": "array",
            "items": {
              "$ref": "#/$defs/Docked"
            }
          },
          "focused_block_id": {
            "type": [
              "string",
              "null"
            ]
          },
          "zoomed_block_id": {
            "description": "The block shown filling the window; the layout is unchanged.",
            "type": [
              "string",
              "null"
            ]
          }
        },
        "required": [
          "window_id",
          "position",
          "active",
          "block_ids",
          "floating",
          "docked"
        ]
      },
      "Node": {
        "oneOf": [
          {
            "type": "object",
            "properties": {
              "block": {
                "type": "object",
                "properties": {
                  "block_id": {
                    "type": "string"
                  }
                },
                "required": [
                  "block_id"
                ]
              }
            },
            "required": [
              "block"
            ],
            "additionalProperties": false
          },
          {
            "type": "object",
            "properties": {
              "split": {
                "type": "object",
                "properties": {
                  "split_id": {
                    "type": "string"
                  },
                  "direction": {
                    "$ref": "#/$defs/Direction"
                  },
                  "ratio": {
                    "description": "The `before` side's share, between 0 and 1.",
                    "type": "number",
                    "format": "double"
                  },
                  "before": {
                    "$ref": "#/$defs/Node"
                  },
                  "after": {
                    "$ref": "#/$defs/Node"
                  }
                },
                "required": [
                  "split_id",
                  "direction",
                  "ratio",
                  "before",
                  "after"
                ]
              }
            },
            "required": [
              "split"
            ],
            "additionalProperties": false
          }
        ]
      },
      "Direction": {
        "oneOf": [
          {
            "description": "Side by side; `before` is on the left.",
            "type": "string",
            "const": "horizontal"
          },
          {
            "description": "Stacked; `before` is on top.",
            "type": "string",
            "const": "vertical"
          }
        ]
      },
      "Floating": {
        "type": "object",
        "properties": {
          "block_id": {
            "type": "string"
          },
          "bounds": {
            "$ref": "#/$defs/Bounds"
          },
          "hidden": {
            "type": "boolean"
          }
        },
        "additionalProperties": false,
        "required": [
          "block_id",
          "bounds",
          "hidden"
        ]
      },
      "Bounds": {
        "type": "object",
        "properties": {
          "x": {
            "type": "number",
            "format": "double"
          },
          "y": {
            "type": "number",
            "format": "double"
          },
          "width": {
            "type": "number",
            "format": "double"
          },
          "height": {
            "type": "number",
            "format": "double"
          }
        },
        "additionalProperties": false,
        "required": [
          "x",
          "y",
          "width",
          "height"
        ]
      },
      "Docked": {
        "type": "object",
        "properties": {
          "block_id": {
            "type": "string"
          },
          "side": {
            "$ref": "#/$defs/DockSide"
          },
          "size": {
            "type": "number",
            "format": "double"
          },
          "hidden": {
            "type": "boolean"
          }
        },
        "additionalProperties": false,
        "required": [
          "block_id",
          "side",
          "size",
          "hidden"
        ]
      },
      "DockSide": {
        "type": "string",
        "enum": [
          "left",
          "right",
          "bottom"
        ]
      }
    }
  }
}
```

</details>

### `session.new_window`

*stable*. Add a window (tab) holding one terminal at the end of a session and make it active.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `window_label` | string \| null |  |  |
| `layout` | `Layout` \| null |  | What the window holds; default a login shell. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `window_id` | string | yes |  |
| `block_id` | string | yes |  |
| `revision` | integer (uint64) | yes |  |

**`Layout`:** { block: `BlockSpec` }. What a new window or split holds: one block (nested layouts aren't supported yet).

**`BlockSpec`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `flavor` | string \| null |  | Only `dev.leon.terminal.shell` exists (the default). |
| `label` | string \| null |  |  |
| `options` | `TerminalOptions` |  |  |

**`TerminalOptions`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `command` | [string] \| null |  | argv to run directly; omitted runs the login shell. |
| `cwd` | string \| null |  | Working directory; defaults to `$HOME`. |
| `exit` | `ExitOptions` \| null |  |  |

**`ExitOptions`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `on_completion` | boolean | yes | Close the block when its program exits (default true); false keeps it open. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NewWindow",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "window_label": {
        "type": [
          "string",
          "null"
        ]
      },
      "layout": {
        "description": "What the window holds; default a login shell.",
        "anyOf": [
          {
            "$ref": "#/$defs/Layout"
          },
          {
            "type": "null"
          }
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id"
    ],
    "$defs": {
      "Layout": {
        "description": "What a new window or split holds: one block (nested layouts aren't supported yet).",
        "oneOf": [
          {
            "type": "object",
            "properties": {
              "block": {
                "$ref": "#/$defs/BlockSpec"
              }
            },
            "required": [
              "block"
            ],
            "additionalProperties": false
          }
        ]
      },
      "BlockSpec": {
        "type": "object",
        "properties": {
          "flavor": {
            "description": "Only `dev.leon.terminal.shell` exists (the default).",
            "type": [
              "string",
              "null"
            ]
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "options": {
            "$ref": "#/$defs/TerminalOptions"
          }
        },
        "additionalProperties": false
      },
      "TerminalOptions": {
        "type": "object",
        "properties": {
          "command": {
            "description": "argv to run directly; omitted runs the login shell.",
            "type": [
              "array",
              "null"
            ],
            "items": {
              "type": "string"
            }
          },
          "cwd": {
            "description": "Working directory; defaults to `$HOME`.",
            "type": [
              "string",
              "null"
            ]
          },
          "exit": {
            "anyOf": [
              {
                "$ref": "#/$defs/ExitOptions"
              },
              {
                "type": "null"
              }
            ]
          }
        },
        "additionalProperties": false
      },
      "ExitOptions": {
        "type": "object",
        "properties": {
          "on_completion": {
            "description": "Close the block when its program exits (default true); false keeps it open.",
            "type": "boolean"
          }
        },
        "additionalProperties": false,
        "required": [
          "on_completion"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "WindowCreated",
    "type": "object",
    "properties": {
      "window_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      },
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "window_id",
      "block_id",
      "revision"
    ]
  }
}
```

</details>

### `session.close_window`

*stable*. Close a window; closing the last one destroys the session.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `window_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "WindowRef",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "window_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "window_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.focus_window`

*stable*. Make a window the session's active tab.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `window_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "WindowRef",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "window_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "window_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.set_window_label`

*stable*. Rename a window.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `window_id` | string | yes |  |
| `label` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SetWindowLabel",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "window_id": {
        "type": "string"
      },
      "label": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "window_id",
      "label"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.list_blocks`

*stable*. A session's blocks in creation order.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `blocks` | [`BlockView`] | yes |  |

**`BlockView`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `label` | string \| null |  |  |
| `window_id` | string | yes |  |
| `pid` | integer (int32) | yes |  |
| `command` | [string] | yes | The argv running (the login shell's path for a shell). |
| `keep_open` | boolean | yes |  |
| `exit_code` | integer (int32) \| null |  | Set once the program exited (only seen on blocks kept open). |
| `title` | string \| null |  | The last title the program set (OSC 0/2). |
| `cwd` | string \| null |  | Where the program is: its cwd, else its last OSC 7 path, else where it started. |
| `foreground` | string \| null |  | The program in front (a shell's job, else the shell): the name of the PTY's foreground process, never its arguments; null once it exited or when another user owns it (`sudo`). Each change is a `foreground_changed` event; this is the value the last one carried (the server looks every 200 ms). |
| `resume` | [string] \| null |  | What runs to resume the program after a restart, if it registered one (`set_resume`). |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SessionRef",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "BlockList",
    "type": "object",
    "properties": {
      "blocks": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/BlockView"
        }
      }
    },
    "required": [
      "blocks"
    ],
    "$defs": {
      "BlockView": {
        "type": "object",
        "properties": {
          "block_id": {
            "type": "string"
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "window_id": {
            "type": "string"
          },
          "pid": {
            "type": "integer",
            "format": "int32"
          },
          "command": {
            "description": "The argv running (the login shell's path for a shell).",
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "keep_open": {
            "type": "boolean"
          },
          "exit_code": {
            "description": "Set once the program exited (only seen on blocks kept open).",
            "type": [
              "integer",
              "null"
            ],
            "format": "int32"
          },
          "title": {
            "description": "The last title the program set (OSC 0/2).",
            "type": [
              "string",
              "null"
            ]
          },
          "cwd": {
            "description": "Where the program is: its cwd, else its last OSC 7 path, else where it started.",
            "type": [
              "string",
              "null"
            ]
          },
          "foreground": {
            "description": "The program in front (a shell's job, else the shell): the name of the PTY's foreground\nprocess, never its arguments; null once it exited or when another user owns it (`sudo`).\nEach change is a `foreground_changed` event; this is the value the last one carried (the\nserver looks every 200 ms).",
            "type": [
              "string",
              "null"
            ]
          },
          "resume": {
            "description": "What runs to resume the program after a restart, if it registered one (`set_resume`).",
            "type": [
              "array",
              "null"
            ],
            "items": {
              "type": "string"
            }
          }
        },
        "required": [
          "block_id",
          "window_id",
          "pid",
          "command",
          "keep_open"
        ]
      }
    }
  }
}
```

</details>

### `session.describe_block`

*stable*. One block: its window, process and exit status.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `label` | string \| null |  |  |
| `window_id` | string | yes |  |
| `pid` | integer (int32) | yes |  |
| `command` | [string] | yes | The argv running (the login shell's path for a shell). |
| `keep_open` | boolean | yes |  |
| `exit_code` | integer (int32) \| null |  | Set once the program exited (only seen on blocks kept open). |
| `title` | string \| null |  | The last title the program set (OSC 0/2). |
| `cwd` | string \| null |  | Where the program is: its cwd, else its last OSC 7 path, else where it started. |
| `foreground` | string \| null |  | The program in front (a shell's job, else the shell): the name of the PTY's foreground process, never its arguments; null once it exited or when another user owns it (`sudo`). Each change is a `foreground_changed` event; this is the value the last one carried (the server looks every 200 ms). |
| `resume` | [string] \| null |  | What runs to resume the program after a restart, if it registered one (`set_resume`). |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "BlockRef",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "BlockView",
    "type": "object",
    "properties": {
      "block_id": {
        "type": "string"
      },
      "label": {
        "type": [
          "string",
          "null"
        ]
      },
      "window_id": {
        "type": "string"
      },
      "pid": {
        "type": "integer",
        "format": "int32"
      },
      "command": {
        "description": "The argv running (the login shell's path for a shell).",
        "type": "array",
        "items": {
          "type": "string"
        }
      },
      "keep_open": {
        "type": "boolean"
      },
      "exit_code": {
        "description": "Set once the program exited (only seen on blocks kept open).",
        "type": [
          "integer",
          "null"
        ],
        "format": "int32"
      },
      "title": {
        "description": "The last title the program set (OSC 0/2).",
        "type": [
          "string",
          "null"
        ]
      },
      "cwd": {
        "description": "Where the program is: its cwd, else its last OSC 7 path, else where it started.",
        "type": [
          "string",
          "null"
        ]
      },
      "foreground": {
        "description": "The program in front (a shell's job, else the shell): the name of the PTY's foreground\nprocess, never its arguments; null once it exited or when another user owns it (`sudo`).\nEach change is a `foreground_changed` event; this is the value the last one carried (the\nserver looks every 200 ms).",
        "type": [
          "string",
          "null"
        ]
      },
      "resume": {
        "description": "What runs to resume the program after a restart, if it registered one (`set_resume`).",
        "type": [
          "array",
          "null"
        ],
        "items": {
          "type": "string"
        }
      }
    },
    "required": [
      "block_id",
      "window_id",
      "pid",
      "command",
      "keep_open"
    ]
  }
}
```

</details>

### `session.new_split`

*stable*. Split a block, a split, or a window's whole layout, and open a block beside it. All or nothing.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `anchor_id` | string \| null |  | The block or split to split; or give `window_id` to split the window's whole layout. |
| `window_id` | string \| null |  |  |
| `direction` | `Direction` | yes |  |
| `side` | `Side` | yes | Which side of the anchor the new block goes. |
| `ratio` | number \| null |  | The before side's share (default 0.5). |
| `layout` | `Layout` | yes |  |
| `focus` | boolean |  | Focus the new block (default false). |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `window_id` | string | yes |  |
| `revision` | integer (uint64) | yes |  |

**`Direction`:** `"horizontal"` | `"vertical"`.

**`Side`:** `"before"` | `"after"`.

**`Layout`:** { block: `BlockSpec` }. What a new window or split holds: one block (nested layouts aren't supported yet).

**`BlockSpec`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `flavor` | string \| null |  | Only `dev.leon.terminal.shell` exists (the default). |
| `label` | string \| null |  |  |
| `options` | `TerminalOptions` |  |  |

**`TerminalOptions`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `command` | [string] \| null |  | argv to run directly; omitted runs the login shell. |
| `cwd` | string \| null |  | Working directory; defaults to `$HOME`. |
| `exit` | `ExitOptions` \| null |  |  |

**`ExitOptions`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `on_completion` | boolean | yes | Close the block when its program exits (default true); false keeps it open. |

**Example:**

```json
{"id":1,"method":"session.new_split","params":{"session_id":"session:5b1c2f0e-8d7a-4c39-9a61-2e4f7b0c9d13","anchor_id":"block:c7d2e816-3a9f-4f05-8b1e-6d0a4c2f9b57","direction":"horizontal","side":"after","layout":{"block":{"label":"logs"}}}}
{"id":1,"result":{"block_id":"block:9e41b0d3-52c8-4a7f-b6e2-0d1f8c3a5e74","window_id":"window:0f3e9a42-7c15-4b8e-a2d6-91c4e5b7f028","revision":4}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NewSplit",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "anchor_id": {
        "description": "The block or split to split; or give `window_id` to split the window's whole layout.",
        "type": [
          "string",
          "null"
        ]
      },
      "window_id": {
        "type": [
          "string",
          "null"
        ]
      },
      "direction": {
        "$ref": "#/$defs/Direction"
      },
      "side": {
        "description": "Which side of the anchor the new block goes.",
        "$ref": "#/$defs/Side"
      },
      "ratio": {
        "description": "The before side's share (default 0.5).",
        "type": [
          "number",
          "null"
        ],
        "format": "double"
      },
      "layout": {
        "$ref": "#/$defs/Layout"
      },
      "focus": {
        "description": "Focus the new block (default false).",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "direction",
      "side",
      "layout"
    ],
    "$defs": {
      "Direction": {
        "oneOf": [
          {
            "description": "Side by side; `before` is on the left.",
            "type": "string",
            "const": "horizontal"
          },
          {
            "description": "Stacked; `before` is on top.",
            "type": "string",
            "const": "vertical"
          }
        ]
      },
      "Side": {
        "type": "string",
        "enum": [
          "before",
          "after"
        ]
      },
      "Layout": {
        "description": "What a new window or split holds: one block (nested layouts aren't supported yet).",
        "oneOf": [
          {
            "type": "object",
            "properties": {
              "block": {
                "$ref": "#/$defs/BlockSpec"
              }
            },
            "required": [
              "block"
            ],
            "additionalProperties": false
          }
        ]
      },
      "BlockSpec": {
        "type": "object",
        "properties": {
          "flavor": {
            "description": "Only `dev.leon.terminal.shell` exists (the default).",
            "type": [
              "string",
              "null"
            ]
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "options": {
            "$ref": "#/$defs/TerminalOptions"
          }
        },
        "additionalProperties": false
      },
      "TerminalOptions": {
        "type": "object",
        "properties": {
          "command": {
            "description": "argv to run directly; omitted runs the login shell.",
            "type": [
              "array",
              "null"
            ],
            "items": {
              "type": "string"
            }
          },
          "cwd": {
            "description": "Working directory; defaults to `$HOME`.",
            "type": [
              "string",
              "null"
            ]
          },
          "exit": {
            "anyOf": [
              {
                "$ref": "#/$defs/ExitOptions"
              },
              {
                "type": "null"
              }
            ]
          }
        },
        "additionalProperties": false
      },
      "ExitOptions": {
        "type": "object",
        "properties": {
          "on_completion": {
            "description": "Close the block when its program exits (default true); false keeps it open.",
            "type": "boolean"
          }
        },
        "additionalProperties": false,
        "required": [
          "on_completion"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SplitCreated",
    "type": "object",
    "properties": {
      "block_id": {
        "type": "string"
      },
      "window_id": {
        "type": "string"
      },
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "block_id",
      "window_id",
      "revision"
    ]
  }
}
```

</details>

### `session.move_block`

*stable*. Put a block beside another in the session; one of another session joins it (`block_closed` there, `block_created` here).

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes | The anchor's session, which the block joins. |
| `block_id` | string | yes | Any block of the server: one of another session moves into this one. |
| `anchor_block_id` | string | yes | The block to put it beside; any window of the session. |
| `direction` | `Direction` \| null |  | Default horizontal. |
| `side` | `Side` \| null |  | Default after. |
| `ratio` | number \| null |  | The before side's share (default 0.5). |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

**`Direction`:** `"horizontal"` | `"vertical"`.

**`Side`:** `"before"` | `"after"`.

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "MoveBlock",
    "type": "object",
    "properties": {
      "session_id": {
        "description": "The anchor's session, which the block joins.",
        "type": "string"
      },
      "block_id": {
        "description": "Any block of the server: one of another session moves into this one.",
        "type": "string"
      },
      "anchor_block_id": {
        "description": "The block to put it beside; any window of the session.",
        "type": "string"
      },
      "direction": {
        "description": "Default horizontal.",
        "anyOf": [
          {
            "$ref": "#/$defs/Direction"
          },
          {
            "type": "null"
          }
        ]
      },
      "side": {
        "description": "Default after.",
        "anyOf": [
          {
            "$ref": "#/$defs/Side"
          },
          {
            "type": "null"
          }
        ]
      },
      "ratio": {
        "description": "The before side's share (default 0.5).",
        "type": [
          "number",
          "null"
        ],
        "format": "double"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id",
      "anchor_block_id"
    ],
    "$defs": {
      "Direction": {
        "oneOf": [
          {
            "description": "Side by side; `before` is on the left.",
            "type": "string",
            "const": "horizontal"
          },
          {
            "description": "Stacked; `before` is on top.",
            "type": "string",
            "const": "vertical"
          }
        ]
      },
      "Side": {
        "type": "string",
        "enum": [
          "before",
          "after"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.swap_blocks`

*stable*. Two blocks of a session trade places.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |
| `other_block_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SwapBlocks",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      },
      "other_block_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id",
      "other_block_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.zoom_block`

*stable*. Show one block filling its window (display only).

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |
| `zoom` | `Zoom` \| null |  | true, false or "toggle" (default). |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

**`Zoom`:** boolean | `ZoomToggle`.

**`ZoomToggle`:** `"toggle"`.

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "ZoomBlock",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      },
      "zoom": {
        "description": "true, false or \"toggle\" (default).",
        "anyOf": [
          {
            "$ref": "#/$defs/Zoom"
          },
          {
            "type": "null"
          }
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id"
    ],
    "$defs": {
      "Zoom": {
        "anyOf": [
          {
            "type": "boolean"
          },
          {
            "description": "`\"toggle\"` (the default).",
            "$ref": "#/$defs/ZoomToggle"
          }
        ]
      },
      "ZoomToggle": {
        "type": "string",
        "enum": [
          "toggle"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.resize_pane`

*stable*. Set the ratio of the split between two blocks.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `direction` | `Direction` | yes | The direction of the split between the two blocks. |
| `before_block_id` | string | yes | A block on the split's before side. |
| `after_block_id` | string | yes | A block on the split's after side. |
| `ratio` | number | yes | The before side's new share, between 0 and 1. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

**`Direction`:** `"horizontal"` | `"vertical"`.

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "ResizePane",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "direction": {
        "description": "The direction of the split between the two blocks.",
        "$ref": "#/$defs/Direction"
      },
      "before_block_id": {
        "description": "A block on the split's before side.",
        "type": "string"
      },
      "after_block_id": {
        "description": "A block on the split's after side.",
        "type": "string"
      },
      "ratio": {
        "description": "The before side's new share, between 0 and 1.",
        "type": "number",
        "format": "double"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "direction",
      "before_block_id",
      "after_block_id",
      "ratio"
    ],
    "$defs": {
      "Direction": {
        "oneOf": [
          {
            "description": "Side by side; `before` is on the left.",
            "type": "string",
            "const": "horizontal"
          },
          {
            "description": "Stacked; `before` is on top.",
            "type": "string",
            "const": "vertical"
          }
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.focus_block`

*stable*. Focus a block and make its window active.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "FocusBlock",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.focus_direction`

*stable*. Move focus left, right, up or down in the active window.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `direction` | `Toward` | yes |  |
| `wrap` | boolean |  | With nothing that way, go to the far side of the window. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `revision` | integer (uint64) | yes |  |

**`Toward`:** `"left"` | `"right"` | `"up"` | `"down"`. Where focus moves to.

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "FocusDirection",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "direction": {
        "$ref": "#/$defs/Toward"
      },
      "wrap": {
        "description": "With nothing that way, go to the far side of the window.",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "direction"
    ],
    "$defs": {
      "Toward": {
        "description": "Where focus moves to.",
        "type": "string",
        "enum": [
          "left",
          "right",
          "up",
          "down"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Focused",
    "type": "object",
    "properties": {
      "block_id": {
        "type": "string"
      },
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "block_id",
      "revision"
    ]
  }
}
```

</details>

### `session.focus_next_pane`

*stable*. Focus the next block of the active window.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `wrap` | boolean |  | Past the last block, go round to the first. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "FocusStep",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "wrap": {
        "description": "Past the last block, go round to the first.",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Focused",
    "type": "object",
    "properties": {
      "block_id": {
        "type": "string"
      },
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "block_id",
      "revision"
    ]
  }
}
```

</details>

### `session.focus_previous_pane`

*stable*. Focus the previous block of the active window.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `wrap` | boolean |  | Past the last block, go round to the first. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "FocusStep",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "wrap": {
        "description": "Past the last block, go round to the first.",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Focused",
    "type": "object",
    "properties": {
      "block_id": {
        "type": "string"
      },
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "block_id",
      "revision"
    ]
  }
}
```

</details>

### `session.float_block`

*stable*. Move a block above its tiled layout; show, focus and raise it without restarting its program.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |
| `bounds` | `Bounds` \| null |  | Fractions of the whole window; default centered 80% by 80%. |
| `label` | string \| null |  |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

**`Bounds`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `x` | number | yes |  |
| `y` | number | yes |  |
| `width` | number | yes |  |
| `height` | number | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Float",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      },
      "bounds": {
        "description": "Fractions of the whole window; default centered 80% by 80%.",
        "anyOf": [
          {
            "$ref": "#/$defs/Bounds"
          },
          {
            "type": "null"
          }
        ]
      },
      "label": {
        "type": [
          "string",
          "null"
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id"
    ],
    "$defs": {
      "Bounds": {
        "type": "object",
        "properties": {
          "x": {
            "type": "number",
            "format": "double"
          },
          "y": {
            "type": "number",
            "format": "double"
          },
          "width": {
            "type": "number",
            "format": "double"
          },
          "height": {
            "type": "number",
            "format": "double"
          }
        },
        "additionalProperties": false,
        "required": [
          "x",
          "y",
          "width",
          "height"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.new_floating`

*stable*. Open a terminal in a visible floating pane in a window (default active).

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `window_id` | string \| null |  | Default active window. |
| `bounds` | `Bounds` \| null |  |  |
| `layout` | `Layout` \| null |  |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `window_id` | string | yes |  |
| `revision` | integer (uint64) | yes |  |

**`Bounds`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `x` | number | yes |  |
| `y` | number | yes |  |
| `width` | number | yes |  |
| `height` | number | yes |  |

**`Layout`:** { block: `BlockSpec` }. What a new window or split holds: one block (nested layouts aren't supported yet).

**`BlockSpec`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `flavor` | string \| null |  | Only `dev.leon.terminal.shell` exists (the default). |
| `label` | string \| null |  |  |
| `options` | `TerminalOptions` |  |  |

**`TerminalOptions`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `command` | [string] \| null |  | argv to run directly; omitted runs the login shell. |
| `cwd` | string \| null |  | Working directory; defaults to `$HOME`. |
| `exit` | `ExitOptions` \| null |  |  |

**`ExitOptions`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `on_completion` | boolean | yes | Close the block when its program exits (default true); false keeps it open. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NewFloating",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "window_id": {
        "description": "Default active window.",
        "type": [
          "string",
          "null"
        ]
      },
      "bounds": {
        "anyOf": [
          {
            "$ref": "#/$defs/Bounds"
          },
          {
            "type": "null"
          }
        ]
      },
      "layout": {
        "anyOf": [
          {
            "$ref": "#/$defs/Layout"
          },
          {
            "type": "null"
          }
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id"
    ],
    "$defs": {
      "Bounds": {
        "type": "object",
        "properties": {
          "x": {
            "type": "number",
            "format": "double"
          },
          "y": {
            "type": "number",
            "format": "double"
          },
          "width": {
            "type": "number",
            "format": "double"
          },
          "height": {
            "type": "number",
            "format": "double"
          }
        },
        "additionalProperties": false,
        "required": [
          "x",
          "y",
          "width",
          "height"
        ]
      },
      "Layout": {
        "description": "What a new window or split holds: one block (nested layouts aren't supported yet).",
        "oneOf": [
          {
            "type": "object",
            "properties": {
              "block": {
                "$ref": "#/$defs/BlockSpec"
              }
            },
            "required": [
              "block"
            ],
            "additionalProperties": false
          }
        ]
      },
      "BlockSpec": {
        "type": "object",
        "properties": {
          "flavor": {
            "description": "Only `dev.leon.terminal.shell` exists (the default).",
            "type": [
              "string",
              "null"
            ]
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "options": {
            "$ref": "#/$defs/TerminalOptions"
          }
        },
        "additionalProperties": false
      },
      "TerminalOptions": {
        "type": "object",
        "properties": {
          "command": {
            "description": "argv to run directly; omitted runs the login shell.",
            "type": [
              "array",
              "null"
            ],
            "items": {
              "type": "string"
            }
          },
          "cwd": {
            "description": "Working directory; defaults to `$HOME`.",
            "type": [
              "string",
              "null"
            ]
          },
          "exit": {
            "anyOf": [
              {
                "$ref": "#/$defs/ExitOptions"
              },
              {
                "type": "null"
              }
            ]
          }
        },
        "additionalProperties": false
      },
      "ExitOptions": {
        "type": "object",
        "properties": {
          "on_completion": {
            "description": "Close the block when its program exits (default true); false keeps it open.",
            "type": "boolean"
          }
        },
        "additionalProperties": false,
        "required": [
          "on_completion"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SplitCreated",
    "type": "object",
    "properties": {
      "block_id": {
        "type": "string"
      },
      "window_id": {
        "type": "string"
      },
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "block_id",
      "window_id",
      "revision"
    ]
  }
}
```

</details>

### `session.dock_block`

*stable*. Pin a block to left, right or bottom; an occupied side fails without changes.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |
| `side` | `DockSide` | yes |  |
| `size` | number \| null |  | Fraction of the window (default 0.3), strictly between zero and one. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

**`DockSide`:** `"left"` | `"right"` | `"bottom"`.

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Dock",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      },
      "side": {
        "$ref": "#/$defs/DockSide"
      },
      "size": {
        "description": "Fraction of the window (default 0.3), strictly between zero and one.",
        "type": [
          "number",
          "null"
        ],
        "format": "double"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id",
      "side"
    ],
    "$defs": {
      "DockSide": {
        "type": "string",
        "enum": [
          "left",
          "right",
          "bottom"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.unfloat_block`

*stable*. Return a float or drawer to the tiled tree, split equally beside its root.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "FocusBlock",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.toggle`

*stable*. Show or hide a labeled float or drawer without stopping its program; optionally create a scratch shell.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `label` | string | yes | Exact case-insensitive block label, unique within this session. |
| `create` | boolean |  | When true and no block has this label, create a login-shell drawer at the bottom of the active window, in the directory of its focused block (else the login shell's default). |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Toggle",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "label": {
        "description": "Exact case-insensitive block label, unique within this session.",
        "type": "string"
      },
      "create": {
        "description": "When true and no block has this label, create a login-shell drawer at the bottom of the active\nwindow, in the directory of its focused block (else the login shell's default).",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "label"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `session.attach`

*stable*. Snapshot a session; on a socket connection, also receive its events (block events too) as `{"event":…,"seq":N}` lines from now on, with seq above the snapshot's.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `resumable` | boolean |  | As in `session.attach_all`. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `seq` | integer (uint64) | yes | The connection's event seq at this snapshot; every later event on it has a higher one. |
| `view` | `SessionView` | yes |  |
| `resume` | string \| null |  | With `resumable`: the token `session.resume` takes, good for every later drop of this stream. |

**`SessionView`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `label` | string \| null |  |  |
| `revision` | integer (uint64) | yes | Bumped by every change to this view. |
| `windows` | [`WindowView`] | yes |  |

**`WindowView`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `window_id` | string | yes |  |
| `label` | string \| null |  |  |
| `position` | integer (uint) | yes | 1-based position in tab order. |
| `active` | boolean | yes |  |
| `block_ids` | [string] | yes | In layout order. |
| `layout` | `Node` \| null |  |  |
| `floating` | [`Floating`] | yes | Back to front, including hidden floats. |
| `docked` | [`Docked`] | yes | At most one panel per side, including hidden drawers. |
| `focused_block_id` | string \| null |  |  |
| `zoomed_block_id` | string \| null |  | The block shown filling the window; the layout is unchanged. |

**`Node`:** { block: { block_id: string } } | { split: { split_id: string, direction: `Direction`, ratio: number, before: `Node`, after: `Node` } }.

**`Direction`:** `"horizontal"` | `"vertical"`.

**`Floating`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `bounds` | `Bounds` | yes |  |
| `hidden` | boolean | yes |  |

**`Bounds`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `x` | number | yes |  |
| `y` | number | yes |  |
| `width` | number | yes |  |
| `height` | number | yes |  |

**`Docked`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes |  |
| `side` | `DockSide` | yes |  |
| `size` | number | yes |  |
| `hidden` | boolean | yes |  |

**`DockSide`:** `"left"` | `"right"` | `"bottom"`.

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "AttachSession",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "resumable": {
        "description": "As in `session.attach_all`.",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Attached",
    "type": "object",
    "properties": {
      "seq": {
        "description": "The connection's event seq at this snapshot; every later event on it has a higher one.",
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      },
      "view": {
        "$ref": "#/$defs/SessionView"
      },
      "resume": {
        "description": "With `resumable`: the token `session.resume` takes, good for every later drop of this stream.",
        "type": [
          "string",
          "null"
        ]
      }
    },
    "required": [
      "seq",
      "view"
    ],
    "$defs": {
      "SessionView": {
        "type": "object",
        "properties": {
          "session_id": {
            "type": "string"
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "revision": {
            "description": "Bumped by every change to this view.",
            "type": "integer",
            "format": "uint64",
            "minimum": 0
          },
          "windows": {
            "type": "array",
            "items": {
              "$ref": "#/$defs/WindowView"
            }
          }
        },
        "required": [
          "session_id",
          "revision",
          "windows"
        ]
      },
      "WindowView": {
        "type": "object",
        "properties": {
          "window_id": {
            "type": "string"
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "position": {
            "description": "1-based position in tab order.",
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "active": {
            "type": "boolean"
          },
          "block_ids": {
            "description": "In layout order.",
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "layout": {
            "anyOf": [
              {
                "$ref": "#/$defs/Node"
              },
              {
                "type": "null"
              }
            ]
          },
          "floating": {
            "description": "Back to front, including hidden floats.",
            "type": "array",
            "items": {
              "$ref": "#/$defs/Floating"
            }
          },
          "docked": {
            "description": "At most one panel per side, including hidden drawers.",
            "type": "array",
            "items": {
              "$ref": "#/$defs/Docked"
            }
          },
          "focused_block_id": {
            "type": [
              "string",
              "null"
            ]
          },
          "zoomed_block_id": {
            "description": "The block shown filling the window; the layout is unchanged.",
            "type": [
              "string",
              "null"
            ]
          }
        },
        "required": [
          "window_id",
          "position",
          "active",
          "block_ids",
          "floating",
          "docked"
        ]
      },
      "Node": {
        "oneOf": [
          {
            "type": "object",
            "properties": {
              "block": {
                "type": "object",
                "properties": {
                  "block_id": {
                    "type": "string"
                  }
                },
                "required": [
                  "block_id"
                ]
              }
            },
            "required": [
              "block"
            ],
            "additionalProperties": false
          },
          {
            "type": "object",
            "properties": {
              "split": {
                "type": "object",
                "properties": {
                  "split_id": {
                    "type": "string"
                  },
                  "direction": {
                    "$ref": "#/$defs/Direction"
                  },
                  "ratio": {
                    "description": "The `before` side's share, between 0 and 1.",
                    "type": "number",
                    "format": "double"
                  },
                  "before": {
                    "$ref": "#/$defs/Node"
                  },
                  "after": {
                    "$ref": "#/$defs/Node"
                  }
                },
                "required": [
                  "split_id",
                  "direction",
                  "ratio",
                  "before",
                  "after"
                ]
              }
            },
            "required": [
              "split"
            ],
            "additionalProperties": false
          }
        ]
      },
      "Direction": {
        "oneOf": [
          {
            "description": "Side by side; `before` is on the left.",
            "type": "string",
            "const": "horizontal"
          },
          {
            "description": "Stacked; `before` is on top.",
            "type": "string",
            "const": "vertical"
          }
        ]
      },
      "Floating": {
        "type": "object",
        "properties": {
          "block_id": {
            "type": "string"
          },
          "bounds": {
            "$ref": "#/$defs/Bounds"
          },
          "hidden": {
            "type": "boolean"
          }
        },
        "additionalProperties": false,
        "required": [
          "block_id",
          "bounds",
          "hidden"
        ]
      },
      "Bounds": {
        "type": "object",
        "properties": {
          "x": {
            "type": "number",
            "format": "double"
          },
          "y": {
            "type": "number",
            "format": "double"
          },
          "width": {
            "type": "number",
            "format": "double"
          },
          "height": {
            "type": "number",
            "format": "double"
          }
        },
        "additionalProperties": false,
        "required": [
          "x",
          "y",
          "width",
          "height"
        ]
      },
      "Docked": {
        "type": "object",
        "properties": {
          "block_id": {
            "type": "string"
          },
          "side": {
            "$ref": "#/$defs/DockSide"
          },
          "size": {
            "type": "number",
            "format": "double"
          },
          "hidden": {
            "type": "boolean"
          }
        },
        "additionalProperties": false,
        "required": [
          "block_id",
          "side",
          "size",
          "hidden"
        ]
      },
      "DockSide": {
        "type": "string",
        "enum": [
          "left",
          "right",
          "bottom"
        ]
      }
    }
  }
}
```

</details>

### `session.attach_all`

*stable*. List sessions; on a socket connection, also receive session lifecycle events (event_scope `session`) or every event (`all`) from now on.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `event_scope` | `EventScope` | yes |  |
| `resumable` | boolean |  | Also return a `resume` token: when this connection drops, `session.resume` on another one carries on with the next seq. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `seq` | integer (uint64) | yes | As in `session.attach`. |
| `sessions` | [`SessionSummary`] | yes |  |
| `resume` | string \| null |  | As in `session.attach`. |

**`EventScope`:** `"session"` | `"all"`.

**`SessionSummary`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `label` | string \| null |  |  |
| `windows` | integer (uint) | yes | Number of windows (tabs). |

**Example:**

```json
{"id":1,"method":"session.attach_all","params":{"event_scope":"session","resumable":true}}
{"id":1,"result":{"seq":0,"sessions":[{"session_id":"session:5b1c2f0e-8d7a-4c39-9a61-2e4f7b0c9d13","label":"web","windows":1}],"resume":"resume:3d8f1a6c-0b2e-4c97-a5d4-7e9b2f1c0a68"}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "AttachAll",
    "type": "object",
    "properties": {
      "event_scope": {
        "$ref": "#/$defs/EventScope"
      },
      "resumable": {
        "description": "Also return a `resume` token: when this connection drops, `session.resume` on another one\ncarries on with the next seq.",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false,
    "required": [
      "event_scope"
    ],
    "$defs": {
      "EventScope": {
        "oneOf": [
          {
            "description": "Session lifecycle: `session_created`, `session_destroyed`.",
            "type": "string",
            "const": "session"
          },
          {
            "description": "Everything, in every session.",
            "type": "string",
            "const": "all"
          }
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "AttachedAll",
    "type": "object",
    "properties": {
      "seq": {
        "description": "As in `session.attach`.",
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      },
      "sessions": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/SessionSummary"
        }
      },
      "resume": {
        "description": "As in `session.attach`.",
        "type": [
          "string",
          "null"
        ]
      }
    },
    "required": [
      "seq",
      "sessions"
    ],
    "$defs": {
      "SessionSummary": {
        "type": "object",
        "properties": {
          "session_id": {
            "type": "string"
          },
          "label": {
            "type": [
              "string",
              "null"
            ]
          },
          "windows": {
            "description": "Number of windows (tabs).",
            "type": "integer",
            "format": "uint",
            "minimum": 0
          }
        },
        "required": [
          "session_id",
          "windows"
        ]
      }
    }
  }
}
```

</details>

### `session.resume`

*stable*. On a socket connection that hasn't attached: take over a resumable event stream (see `session.attach_all`) and get its events after `seq` again, then the new ones, with the same seq counter. Fails with `resume_failed` when the stream is gone or `seq` is no longer kept; then attach again.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `resume` | string | yes | The token `session.attach` or `session.attach_all` returned with `resumable`. |
| `seq` | integer (uint64) | yes | The last seq the client got; events come again from the next one. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `seq` | integer (uint64) | yes | The seq resumed after. |

**Example:**

```json
{"id":1,"method":"session.resume","params":{"resume":"resume:3d8f1a6c-0b2e-4c97-a5d4-7e9b2f1c0a68","seq":41}}
{"id":1,"result":{"seq":41}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Resume",
    "type": "object",
    "properties": {
      "resume": {
        "description": "The token `session.attach` or `session.attach_all` returned with `resumable`.",
        "type": "string"
      },
      "seq": {
        "description": "The last seq the client got; events come again from the next one.",
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "additionalProperties": false,
    "required": [
      "resume",
      "seq"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Resumed",
    "type": "object",
    "properties": {
      "seq": {
        "description": "The seq resumed after.",
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "seq"
    ]
  }
}
```

</details>

## target

### `target.resolve`

*stable*. Turn a query (ID, label, position or ID suffix) into one ID; ambiguous queries fail with every match.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `kind` | `TargetKind` | yes |  |
| `query` | string | yes | A full ID, a label (case-insensitive), a 1-based position, or an ID suffix of 4+ characters. |
| `session_id` | string \| null |  | Required for `window`: the session to look in. For `block`, omitted searches every session. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes |  |
| `session_id` | string \| null |  | For a block: the session it belongs to. |

**`TargetKind`:** `"session"` | `"window"` | `"block"`.

**Example:**

```json
{"id":1,"method":"target.resolve","params":{"kind":"session","query":"web"}}
{"id":1,"result":{"id":"session:5b1c2f0e-8d7a-4c39-9a61-2e4f7b0c9d13","session_id":null}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Resolve",
    "type": "object",
    "properties": {
      "kind": {
        "$ref": "#/$defs/TargetKind"
      },
      "query": {
        "description": "A full ID, a label (case-insensitive), a 1-based position, or an ID suffix of 4+ characters.",
        "type": "string"
      },
      "session_id": {
        "description": "Required for `window`: the session to look in. For `block`, omitted searches every session.",
        "type": [
          "string",
          "null"
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "kind",
      "query"
    ],
    "$defs": {
      "TargetKind": {
        "type": "string",
        "enum": [
          "session",
          "window",
          "block"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Resolved",
    "type": "object",
    "properties": {
      "id": {
        "type": "string"
      },
      "session_id": {
        "description": "For a block: the session it belongs to.",
        "type": [
          "string",
          "null"
        ]
      }
    },
    "required": [
      "id"
    ]
  }
}
```

</details>

## block

### `block.close`

*stable*. Close a block and end its program; its split collapses, and an emptied window closes too.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `revision` | integer (uint64) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "BlockRef",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Revision",
    "description": "What a call that changes a session's view returns.",
    "type": "object",
    "properties": {
      "revision": {
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "revision"
    ]
  }
}
```

</details>

### `block.method`

*stable*. Call a block method (see block_types in help.all) with `args`.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |
| `method` | string | yes | A block method such as `dev.leon.terminal.write`. |
| `args` | any |  | The method's own arguments. |

**Output:** any

**Example:**

```json
{"id":1,"method":"block.method","params":{"session_id":"session:5b1c2f0e-8d7a-4c39-9a61-2e4f7b0c9d13","block_id":"block:c7d2e816-3a9f-4f05-8b1e-6d0a4c2f9b57","method":"dev.leon.terminal.format","args":{"trim":true}}}
{"id":1,"result":{"text":"$ ls\nREADME.md  src\n$"}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "CallMethod",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      },
      "method": {
        "description": "A block method such as `dev.leon.terminal.write`.",
        "type": "string"
      },
      "args": {
        "description": "The method's own arguments."
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id",
      "method"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "AnyValue"
  }
}
```

</details>

### `block.event`

*stable*. Send an input event to a block, bypassing key bindings.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `block_id` | string | yes |  |
| `event` | `InputEvent` | yes |  |

**Output:** none

**`InputEvent`:** { type: `"key"`, key: string } | { type: `"focus"` } | { type: `"paste"`, data: string } | { type: `"mouse"`, action: `Action`, button?: `Button` | null, line: integer (uint), col: integer (uint), mods?: `Mods` } | { type: `"wheel"`, lines: integer (int32), line: integer (uint), col: integer (uint), mods?: `Mods` }.

**`Action`:** `"press"` | `"release"` | `"move"`.

**`Button`:** `"left"` | `"middle"` | `"right"` | `"wheel_up"` | `"wheel_down"`.

**`Mods`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `shift` | boolean |  |  |
| `alt` | boolean |  |  |
| `ctrl` | boolean |  |  |

**Example:**

```json
{"id":1,"method":"block.event","params":{"session_id":"session:5b1c2f0e-8d7a-4c39-9a61-2e4f7b0c9d13","block_id":"block:c7d2e816-3a9f-4f05-8b1e-6d0a4c2f9b57","event":{"type":"key","key":"ctrl+c"}}}
{"id":1,"result":{}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SendEvent",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "block_id": {
        "type": "string"
      },
      "event": {
        "$ref": "#/$defs/InputEvent"
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "block_id",
      "event"
    ],
    "$defs": {
      "InputEvent": {
        "oneOf": [
          {
            "description": "Press a key given in binding syntax (`enter`, `ctrl+c`, `alt+left`, `a`).",
            "type": "object",
            "properties": {
              "type": {
                "type": "string",
                "const": "key"
              },
              "key": {
                "type": "string"
              }
            },
            "additionalProperties": false,
            "required": [
              "type",
              "key"
            ]
          },
          {
            "description": "The user arrived at the block: its `done` and `error` status records are cleared (reason `seen`).",
            "type": "object",
            "properties": {
              "type": {
                "type": "string",
                "const": "focus"
              }
            },
            "required": [
              "type"
            ],
            "additionalProperties": false
          },
          {
            "description": "Paste text: line breaks become Enter, and if the program asked for bracketed paste the text\nis wrapped in its markers (with ESC and C1 controls removed), by the mode it has now.",
            "type": "object",
            "properties": {
              "type": {
                "type": "string",
                "const": "paste"
              },
              "data": {
                "type": "string"
              }
            },
            "additionalProperties": false,
            "required": [
              "type",
              "data"
            ]
          },
          {
            "description": "A mouse press, release or move on a screen cell (0-based), reported to the program in the\nmode and encoding it asked for now (X10, 1000, 1002, 1003; SGR, UTF-8 or legacy); nothing\nwhen it asked for none. `button` null is a move with no button down.",
            "type": "object",
            "properties": {
              "type": {
                "type": "string",
                "const": "mouse"
              },
              "action": {
                "$ref": "#/$defs/Action"
              },
              "button": {
                "anyOf": [
                  {
                    "$ref": "#/$defs/Button"
                  },
                  {
                    "type": "null"
                  }
                ]
              },
              "line": {
                "type": "integer",
                "format": "uint",
                "minimum": 0
              },
              "col": {
                "type": "integer",
                "format": "uint",
                "minimum": 0
              },
              "mods": {
                "$ref": "#/$defs/Mods",
                "default": {
                  "shift": false,
                  "alt": false,
                  "ctrl": false
                }
              }
            },
            "additionalProperties": false,
            "required": [
              "type",
              "action",
              "line",
              "col"
            ]
          },
          {
            "description": "The wheel, `lines` notches up (down when negative, at most 100): reported as buttons 64/65,\nor on the alternate screen with no mouse mode as arrow keys.",
            "type": "object",
            "properties": {
              "type": {
                "type": "string",
                "const": "wheel"
              },
              "lines": {
                "type": "integer",
                "format": "int32"
              },
              "line": {
                "type": "integer",
                "format": "uint",
                "minimum": 0
              },
              "col": {
                "type": "integer",
                "format": "uint",
                "minimum": 0
              },
              "mods": {
                "$ref": "#/$defs/Mods",
                "default": {
                  "shift": false,
                  "alt": false,
                  "ctrl": false
                }
              }
            },
            "additionalProperties": false,
            "required": [
              "type",
              "lines",
              "line",
              "col"
            ]
          }
        ]
      },
      "Action": {
        "type": "string",
        "enum": [
          "press",
          "release",
          "move"
        ]
      },
      "Button": {
        "type": "string",
        "enum": [
          "left",
          "middle",
          "right",
          "wheel_up",
          "wheel_down"
        ]
      },
      "Mods": {
        "type": "object",
        "properties": {
          "shift": {
            "type": "boolean",
            "default": false
          },
          "alt": {
            "type": "boolean",
            "default": false
          },
          "ctrl": {
            "type": "boolean",
            "default": false
          }
        },
        "additionalProperties": false
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Empty",
    "type": "object"
  }
}
```

</details>

### `block.wait`

*stable*. Wait for a terminal's program to exit and return its status (128 + signal if killed).

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `block_id` | string | yes | The block's full ID; works after the block closed, too. |
| `timeout_ms` | integer (uint64) \| null |  | Give up after this long with error `timeout`; omitted waits for ever. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `exit_code` | integer (int32) | yes |  |

**Example:**

```json
{"id":1,"method":"block.wait","params":{"block_id":"block:c7d2e816-3a9f-4f05-8b1e-6d0a4c2f9b57","timeout_ms":60000}}
{"id":1,"result":{"exit_code":0}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Wait",
    "type": "object",
    "properties": {
      "block_id": {
        "description": "The block's full ID; works after the block closed, too.",
        "type": "string"
      },
      "timeout_ms": {
        "description": "Give up after this long with error `timeout`; omitted waits for ever.",
        "type": [
          "integer",
          "null"
        ],
        "format": "uint64",
        "minimum": 0
      }
    },
    "additionalProperties": false,
    "required": [
      "block_id"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Exited",
    "type": "object",
    "properties": {
      "exit_code": {
        "type": "integer",
        "format": "int32"
      }
    },
    "required": [
      "exit_code"
    ]
  }
}
```

</details>

### `block.watch`

*experimental*. On a socket connection, receive these blocks' screens as `{"screen":{block_id, full, lines, cols, rows, cursor, app_cursor, bracketed_paste}}` lines: a whole screen first (`full`), then only the rows that changed. No seq: each line is a diff against the last one for that block. Replaces the set watched before; `[]` stops. `palette` sets the colours this connection's screens are resolved with (leon-dark's by default); a new one sends every screen again, whole.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `block_ids` | [string] | yes | The blocks whose screens this connection gets, replacing the last set; `[]` stops them all. |
| `palette` | `Palette` \| null |  | The colours to resolve this connection's screens with [default: leon-dark's]. A new palette sends every watched screen again, whole. |

**Output:** none

**`Palette`:** A client's terminal colours, as 0xRRGGBB numbers: the default text and background, the cursor and ANSI 0–15.

| Field | Type | Required | Description |
|---|---|---|---|
| `fg` | integer (uint32) | yes |  |
| `bg` | integer (uint32) | yes |  |
| `cursor` | integer (uint32) | yes |  |
| `ansi` | [integer (uint32)] | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Watch",
    "type": "object",
    "properties": {
      "block_ids": {
        "description": "The blocks whose screens this connection gets, replacing the last set; `[]` stops them all.",
        "type": "array",
        "items": {
          "type": "string"
        }
      },
      "palette": {
        "description": "The colours to resolve this connection's screens with [default: leon-dark's]. A new palette\nsends every watched screen again, whole.",
        "anyOf": [
          {
            "$ref": "#/$defs/Palette"
          },
          {
            "type": "null"
          }
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "block_ids"
    ],
    "$defs": {
      "Palette": {
        "description": "A client's terminal colours, as 0xRRGGBB numbers: the default text and background, the cursor\nand ANSI 0–15.",
        "type": "object",
        "properties": {
          "fg": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          },
          "bg": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          },
          "cursor": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          },
          "ansi": {
            "type": "array",
            "items": {
              "type": "integer",
              "format": "uint32",
              "minimum": 0
            },
            "minItems": 16,
            "maxItems": 16
          }
        },
        "additionalProperties": false,
        "required": [
          "fg",
          "bg",
          "cursor",
          "ansi"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Empty",
    "type": "object"
  }
}
```

</details>

## client

### `client.action`

*experimental*. Run a config action (`leon.action` in init.lua) on the server and return what it returned. It gets 30 s; `error(msg, 0)` fails it with msg.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `action` | string | yes | A config action's name. |
| `args` | any |  |  |
| `origin` | string \| null |  | `ctx.origin` for the action (default `api`). |
| `session_id` | string \| null |  |  |
| `block_id` | string \| null |  |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `result` | any | yes | What the action's `run` returned. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "ActionCall",
    "type": "object",
    "properties": {
      "action": {
        "description": "A config action's name.",
        "type": "string"
      },
      "args": true,
      "origin": {
        "description": "`ctx.origin` for the action (default `api`).",
        "type": [
          "string",
          "null"
        ]
      },
      "session_id": {
        "type": [
          "string",
          "null"
        ]
      },
      "block_id": {
        "type": [
          "string",
          "null"
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "action"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "ActionResult",
    "type": "object",
    "properties": {
      "result": {
        "description": "What the action's `run` returned."
      }
    },
    "required": [
      "result"
    ]
  }
}
```

</details>

### `client.key`

*experimental*. Press a key through the config's bindings: an action binding runs, a function binding runs and handles it if it returns true, a sequence's prefix replies `pending`; an unhandled key is typed into the block. Modes are not supported yet.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `session_id` | string | yes |  |
| `key` | string | yes | One key in binding syntax (`ctrl+x`). |
| `block_id` | string \| null |  | The block that has the key; default the active window's focused block. |
| `prefix` | string \| null |  | The keys of a sequence pressed so far (`ctrl+b`), from a `pending` reply. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `handled` | boolean | yes | A binding took the key. |
| `pending` | boolean | yes | The keys so far start a longer sequence: send the next with `prefix`. |
| `prefix` | string \| null |  |  |
| `action` | string \| null |  | The action that ran, for an action binding. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "KeyPress",
    "type": "object",
    "properties": {
      "session_id": {
        "type": "string"
      },
      "key": {
        "description": "One key in binding syntax (`ctrl+x`).",
        "type": "string"
      },
      "block_id": {
        "description": "The block that has the key; default the active window's focused block.",
        "type": [
          "string",
          "null"
        ]
      },
      "prefix": {
        "description": "The keys of a sequence pressed so far (`ctrl+b`), from a `pending` reply.",
        "type": [
          "string",
          "null"
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "session_id",
      "key"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "KeyResult",
    "type": "object",
    "properties": {
      "handled": {
        "description": "A binding took the key.",
        "type": "boolean"
      },
      "pending": {
        "description": "The keys so far start a longer sequence: send the next with `prefix`.",
        "type": "boolean"
      },
      "prefix": {
        "type": [
          "string",
          "null"
        ]
      },
      "action": {
        "description": "The action that ran, for an action binding.",
        "type": [
          "string",
          "null"
        ]
      }
    },
    "required": [
      "handled",
      "pending"
    ]
  }
}
```

</details>

### `client.keymap`

*internal*. The server's resolved key bindings, modes and config actions, with the keymap generation.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `generation` | integer (uint64) | yes | Bumped by every good load. |
| `path` | string \| null |  |  |
| `bindings` | [`BindingView`] | yes | Oldest first; for the same key the newest wins. |
| `modes` | [`ModeView`] | yes |  |
| `server_actions` | [`ActionView`] | yes |  |
| `unbinds` | [`UnbindView`] | yes | `leon.unbind` calls, for a client to apply to its own default bindings. |
| `settings` | `ClientSettings` | yes | The config's client settings (`leon.set`). |

**`BindingView`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `key` | string | yes | The chords, without the mode (`ctrl+b>c`). |
| `mode` | string \| null |  |  |
| `action` | string \| null |  |  |
| `args` | any |  |  |
| `fn` | boolean | yes | Bound to a Lua function. |
| `repeats` | boolean | yes | After the sequence, its last key works again without the prefix for `repeat_time`. |
| `source` | string | yes | `file:line`. |

**`ModeView`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes |  |
| `exclusive` | boolean | yes |  |
| `prefix` | string \| null |  | The chord that enters the mode for one key (`leon attach`'s prefix), if set. |
| `source` | string | yes |  |

**`ActionView`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes |  |
| `title` | string | yes |  |
| `description` | string \| null |  |  |
| `category` | string | yes |  |
| `keywords` | [string] | yes |  |
| `args` | any |  | JSON Schema of the arguments. |
| `repeats` | boolean | yes |  |
| `source` | string | yes |  |

**`UnbindView`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `key` | string | yes | The chords, without the mode. |
| `mode` | string \| null |  |  |
| `action` | string \| null |  | Only bindings to this action; none = every binding of the key. |

**`ClientSettings`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `copy_on_select` | boolean | yes | Copy a selection to the clipboard as it is made (default false). |
| `repeat_time` | integer (uint64) | yes | Milliseconds after a repeatable binding (`repeats`) during which its keys work without the prefix (default 500; 0 turns repeat off). |
| `hint_patterns` | [string] | yes | Regular expressions hints match besides the built-in ones (group 1, if any, is the text). |
| `notifications` | `Notifications` | yes | Which notifications the app posts (`notifications.*`, default all). |

**`Notifications`:** Which notifications the app posts.

| Field | Type | Required | Description |
|---|---|---|---|
| `done` | boolean | yes | A long run finished while the app was in the background. |
| `blocked` | boolean | yes | A block became blocked. |
| `error` | boolean | yes | A block reported an error. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoParams",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Keymap",
    "type": "object",
    "properties": {
      "generation": {
        "description": "Bumped by every good load.",
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      },
      "path": {
        "type": [
          "string",
          "null"
        ]
      },
      "bindings": {
        "description": "Oldest first; for the same key the newest wins.",
        "type": "array",
        "items": {
          "$ref": "#/$defs/BindingView"
        }
      },
      "modes": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/ModeView"
        }
      },
      "server_actions": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/ActionView"
        }
      },
      "unbinds": {
        "description": "`leon.unbind` calls, for a client to apply to its own default bindings.",
        "type": "array",
        "items": {
          "$ref": "#/$defs/UnbindView"
        }
      },
      "settings": {
        "description": "The config's client settings (`leon.set`).",
        "$ref": "#/$defs/ClientSettings"
      }
    },
    "required": [
      "generation",
      "bindings",
      "modes",
      "server_actions",
      "unbinds",
      "settings"
    ],
    "$defs": {
      "BindingView": {
        "type": "object",
        "properties": {
          "key": {
            "description": "The chords, without the mode (`ctrl+b>c`).",
            "type": "string"
          },
          "mode": {
            "type": [
              "string",
              "null"
            ]
          },
          "action": {
            "type": [
              "string",
              "null"
            ]
          },
          "args": true,
          "fn": {
            "description": "Bound to a Lua function.",
            "type": "boolean"
          },
          "repeats": {
            "description": "After the sequence, its last key works again without the prefix for `repeat_time`.",
            "type": "boolean"
          },
          "source": {
            "description": "`file:line`.",
            "type": "string"
          }
        },
        "required": [
          "key",
          "fn",
          "repeats",
          "source"
        ]
      },
      "ModeView": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string"
          },
          "exclusive": {
            "type": "boolean"
          },
          "prefix": {
            "description": "The chord that enters the mode for one key (`leon attach`'s prefix), if set.",
            "type": [
              "string",
              "null"
            ]
          },
          "source": {
            "type": "string"
          }
        },
        "required": [
          "name",
          "exclusive",
          "source"
        ]
      },
      "ActionView": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string"
          },
          "title": {
            "type": "string"
          },
          "description": {
            "type": [
              "string",
              "null"
            ]
          },
          "category": {
            "type": "string"
          },
          "keywords": {
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "args": {
            "description": "JSON Schema of the arguments."
          },
          "repeats": {
            "type": "boolean"
          },
          "source": {
            "type": "string"
          }
        },
        "required": [
          "name",
          "title",
          "category",
          "keywords",
          "repeats",
          "source"
        ]
      },
      "UnbindView": {
        "type": "object",
        "properties": {
          "key": {
            "description": "The chords, without the mode.",
            "type": "string"
          },
          "mode": {
            "type": [
              "string",
              "null"
            ]
          },
          "action": {
            "description": "Only bindings to this action; none = every binding of the key.",
            "type": [
              "string",
              "null"
            ]
          }
        },
        "required": [
          "key"
        ]
      },
      "ClientSettings": {
        "type": "object",
        "properties": {
          "copy_on_select": {
            "description": "Copy a selection to the clipboard as it is made (default false).",
            "type": "boolean"
          },
          "repeat_time": {
            "description": "Milliseconds after a repeatable binding (`repeats`) during which its keys work without the\nprefix (default 500; 0 turns repeat off).",
            "type": "integer",
            "format": "uint64",
            "minimum": 0
          },
          "hint_patterns": {
            "description": "Regular expressions hints match besides the built-in ones (group 1, if any, is the text).",
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "notifications": {
            "description": "Which notifications the app posts (`notifications.*`, default all).",
            "$ref": "#/$defs/Notifications"
          }
        },
        "required": [
          "copy_on_select",
          "repeat_time",
          "hint_patterns",
          "notifications"
        ]
      },
      "Notifications": {
        "description": "Which notifications the app posts.",
        "type": "object",
        "properties": {
          "done": {
            "description": "A long run finished while the app was in the background.",
            "type": "boolean"
          },
          "blocked": {
            "description": "A block became blocked.",
            "type": "boolean"
          },
          "error": {
            "description": "A block reported an error.",
            "type": "boolean"
          }
        },
        "required": [
          "done",
          "blocked",
          "error"
        ]
      }
    }
  }
}
```

</details>

## Block methods: dev.leon.terminal

A block made by `dev.leon.terminal` (flavors: `dev.leon.terminal.shell`) answers these through `block.method`: pass
the method's full name as `method` and its input as `args`. The output is `block.method`'s result.

### `dev.leon.terminal.write`

*stable*. Type text into the terminal.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `data` | string | yes | Bytes to type, as UTF-8 text; nothing is added (no Enter). |

**Output:** none

**Example:**

```json
{"id":1,"method":"dev.leon.terminal.write","params":{"data":"ls\r"}}
{"id":1,"result":{}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "WriteArgs",
    "type": "object",
    "properties": {
      "data": {
        "description": "Bytes to type, as UTF-8 text; nothing is added (no Enter).",
        "type": "string"
      }
    },
    "additionalProperties": false,
    "required": [
      "data"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Empty",
    "type": "object"
  }
}
```

</details>

### `dev.leon.terminal.format`

*stable*. The screen and scrollback as text.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `format` | string \| null |  | Only `text` so far. |
| `trim` | boolean |  | Drop trailing spaces and trailing blank lines. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `text` | string | yes | Scrollback then screen, oldest line first. |

**Example:**

```json
{"id":1,"method":"dev.leon.terminal.format","params":{"format":"text","trim":true}}
{"id":1,"result":{"text":"$ ls\nREADME.md  src\n$"}}
```

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "FormatArgs",
    "type": "object",
    "properties": {
      "format": {
        "description": "Only `text` so far.",
        "type": [
          "string",
          "null"
        ]
      },
      "trim": {
        "description": "Drop trailing spaces and trailing blank lines.",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Formatted",
    "type": "object",
    "properties": {
      "text": {
        "description": "Scrollback then screen, oldest line first.",
        "type": "string"
      }
    },
    "required": [
      "text"
    ]
  }
}
```

</details>

### `dev.leon.terminal.size`

*stable*. Rows and columns.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `rows` | integer (uint16) | yes |  |
| `cols` | integer (uint16) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoArgs",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "TermSize",
    "type": "object",
    "properties": {
      "rows": {
        "type": "integer",
        "format": "uint16",
        "minimum": 0,
        "maximum": 65535
      },
      "cols": {
        "type": "integer",
        "format": "uint16",
        "minimum": 0,
        "maximum": 65535
      }
    },
    "required": [
      "rows",
      "cols"
    ]
  }
}
```

</details>

### `dev.leon.terminal.resize`

*stable*. Resize the grid and the PTY (at least 1 row, 2 columns); returns the new size.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `rows` | integer (uint16) | yes |  |
| `cols` | integer (uint16) | yes |  |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `rows` | integer (uint16) | yes |  |
| `cols` | integer (uint16) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "ResizeArgs",
    "type": "object",
    "properties": {
      "rows": {
        "type": "integer",
        "format": "uint16",
        "minimum": 0,
        "maximum": 65535
      },
      "cols": {
        "type": "integer",
        "format": "uint16",
        "minimum": 0,
        "maximum": 65535
      }
    },
    "additionalProperties": false,
    "required": [
      "rows",
      "cols"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "TermSize",
    "type": "object",
    "properties": {
      "rows": {
        "type": "integer",
        "format": "uint16",
        "minimum": 0,
        "maximum": 65535
      },
      "cols": {
        "type": "integer",
        "format": "uint16",
        "minimum": 0,
        "maximum": 65535
      }
    },
    "required": [
      "rows",
      "cols"
    ]
  }
}
```

</details>

### `dev.leon.terminal.pwd`

*stable*. Where the program is: its working directory, else its last OSC 7 path.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `path` | string \| null |  | The program's working directory, else the last OSC 7 path, else where it started; null if unknown. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoArgs",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Pwd",
    "type": "object",
    "properties": {
      "path": {
        "description": "The program's working directory, else the last OSC 7 path, else where it started; null if unknown.",
        "type": [
          "string",
          "null"
        ]
      }
    }
  }
}
```

</details>

### `dev.leon.terminal.screen`

*experimental*. The screen as `block.watch` pushes it, whole: styled spans with RGB colours per row, backgrounds, the cursor, and the input modes.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `full` | boolean | yes |  |
| `lines` | integer (uint) | yes |  |
| `cols` | integer (uint) | yes |  |
| `rows` | [`Row`] | yes |  |
| `cursor` | `Cursor` \| null |  |  |
| `app_cursor` | boolean | yes |  |
| `bracketed_paste` | boolean | yes |  |
| `scrolled` | integer (uint) |  | The absolute number of the screen's top row: how many lines were ever pushed into the scrollback. History line `scrolled - 1` is just above the screen; appended output never renumbers a line (docs/architecture.md, "Scrollback in the app"). |
| `history` | integer (uint) |  | Lines kept in the scrollback: the oldest is `scrolled - history`. |
| `alt_screen` | boolean |  |  |
| `alternate_scroll` | boolean |  | `?1007`: on the alternate screen the wheel becomes arrow keys. |
| `mouse` | `Report` \| null |  | The mouse events the program asked for, if any. |

**`Row`:** One row of a screen update: its spans and its backgrounds (without the line).

| Field | Type | Required | Description |
|---|---|---|---|
| `line` | integer (uint) | yes |  |
| `spans` | [`Span`] | yes |  |
| `backgrounds` | [`Bg`] | yes |  |

**`Span`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `col` | integer (uint) | yes |  |
| `cells` | integer (uint) | yes | Columns covered (a wide char counts two, combining marks none). |
| `text` | string | yes |  |
| `fg` | integer (uint32) | yes |  |
| `bold` | boolean | yes |  |
| `italic` | boolean | yes |  |
| `underline` | boolean | yes |  |
| `strikeout` | boolean | yes |  |

**`Bg`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `col` | integer (uint) | yes |  |
| `len` | integer (uint) | yes |  |
| `color` | integer (uint32) | yes |  |

**`Cursor`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `line` | integer (uint) | yes |  |
| `col` | integer (uint) | yes |  |
| `shape` | string | yes | `block`, `underline`, `beam`, `hollow` or `hidden`. |
| `wide` | boolean | yes | The cell is a wide char, so a block cursor covers two columns. |
| `text` | string | yes | What the cell holds, redrawn in the background color over a block cursor. |

**`Report`:** `"x10"` | `"click"` | `"drag"` | `"motion"`. Which mouse events a program asked for.

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoArgs",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Update",
    "description": "What a watcher is sent for one terminal: every row when `full` (the first screen, or a new\nsize), else only the rows that changed. The cursor and modes always come along.",
    "type": "object",
    "properties": {
      "full": {
        "type": "boolean"
      },
      "lines": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "cols": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "rows": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/Row"
        }
      },
      "cursor": {
        "anyOf": [
          {
            "$ref": "#/$defs/Cursor"
          },
          {
            "type": "null"
          }
        ]
      },
      "app_cursor": {
        "type": "boolean"
      },
      "bracketed_paste": {
        "type": "boolean"
      },
      "scrolled": {
        "description": "The absolute number of the screen's top row: how many lines were ever pushed into the\nscrollback. History line `scrolled - 1` is just above the screen; appended output never\nrenumbers a line (docs/architecture.md, \"Scrollback in the app\").",
        "type": "integer",
        "format": "uint",
        "minimum": 0,
        "default": 0
      },
      "history": {
        "description": "Lines kept in the scrollback: the oldest is `scrolled - history`.",
        "type": "integer",
        "format": "uint",
        "minimum": 0,
        "default": 0
      },
      "alt_screen": {
        "type": "boolean",
        "default": false
      },
      "alternate_scroll": {
        "description": "`?1007`: on the alternate screen the wheel becomes arrow keys.",
        "type": "boolean",
        "default": false
      },
      "mouse": {
        "description": "The mouse events the program asked for, if any.",
        "anyOf": [
          {
            "$ref": "#/$defs/Report"
          },
          {
            "type": "null"
          }
        ],
        "default": null
      }
    },
    "required": [
      "full",
      "lines",
      "cols",
      "rows",
      "app_cursor",
      "bracketed_paste"
    ],
    "$defs": {
      "Row": {
        "description": "One row of a screen update: its spans and its backgrounds (without the line).",
        "type": "object",
        "properties": {
          "line": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "spans": {
            "type": "array",
            "items": {
              "$ref": "#/$defs/Span"
            }
          },
          "backgrounds": {
            "type": "array",
            "items": {
              "$ref": "#/$defs/Bg"
            }
          }
        },
        "required": [
          "line",
          "spans",
          "backgrounds"
        ]
      },
      "Span": {
        "type": "object",
        "properties": {
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "cells": {
            "description": "Columns covered (a wide char counts two, combining marks none).",
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "text": {
            "type": "string"
          },
          "fg": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          },
          "bold": {
            "type": "boolean"
          },
          "italic": {
            "type": "boolean"
          },
          "underline": {
            "type": "boolean"
          },
          "strikeout": {
            "type": "boolean"
          }
        },
        "required": [
          "col",
          "cells",
          "text",
          "fg",
          "bold",
          "italic",
          "underline",
          "strikeout"
        ]
      },
      "Bg": {
        "type": "object",
        "properties": {
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "len": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "color": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          }
        },
        "required": [
          "col",
          "len",
          "color"
        ]
      },
      "Cursor": {
        "type": "object",
        "properties": {
          "line": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "shape": {
            "description": "`block`, `underline`, `beam`, `hollow` or `hidden`.",
            "type": "string"
          },
          "wide": {
            "description": "The cell is a wide char, so a block cursor covers two columns.",
            "type": "boolean"
          },
          "text": {
            "description": "What the cell holds, redrawn in the background color over a block cursor.",
            "type": "string"
          }
        },
        "required": [
          "line",
          "col",
          "shape",
          "wide",
          "text"
        ]
      },
      "Report": {
        "description": "Which mouse events a program asked for.",
        "oneOf": [
          {
            "description": "`?9`: presses only, no modifiers.",
            "type": "string",
            "const": "x10"
          },
          {
            "description": "`?1000`: presses and releases.",
            "type": "string",
            "const": "click"
          },
          {
            "description": "`?1002`: those, and moves while a button is down.",
            "type": "string",
            "const": "drag"
          },
          {
            "description": "`?1003`: every move.",
            "type": "string",
            "const": "motion"
          }
        ]
      }
    }
  }
}
```

</details>

### `dev.leon.terminal.process`

*stable*. The program's pid, argv, and how much output was parsed.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `pid` | integer (int32) | yes |  |
| `command` | [string] | yes |  |
| `output_bytes` | integer (uint64) | yes | Bytes of output the terminal has parsed (as the PTY delivered them: `\n` arrives as `\r\n`). |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoArgs",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Process",
    "type": "object",
    "properties": {
      "pid": {
        "type": "integer",
        "format": "int32"
      },
      "command": {
        "type": "array",
        "items": {
          "type": "string"
        }
      },
      "output_bytes": {
        "description": "Bytes of output the terminal has parsed (as the PTY delivered them: `\\n` arrives as `\\r\\n`).",
        "type": "integer",
        "format": "uint64",
        "minimum": 0
      }
    },
    "required": [
      "pid",
      "command",
      "output_bytes"
    ]
  }
}
```

</details>

### `dev.leon.terminal.set_resume`

*stable*. Register the command that resumes this block's program after a server restart. Restored blocks type it at the new prompt (setting `resume`: ask, always or never); nothing else that ran is run again.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `command` | [string] \| null |  | argv that resumes this program after a server restart, e.g. `["claude", "--resume", ID]`; null (or leaving it out) clears it. At most 64 arguments and 8 KiB of printable ASCII, no backslash. |

**Output:** none

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SetResume",
    "type": "object",
    "properties": {
      "command": {
        "description": "argv that resumes this program after a server restart, e.g. `[\"claude\", \"--resume\", ID]`;\nnull (or leaving it out) clears it. At most 64 arguments and 8 KiB of printable ASCII, no\nbackslash.",
        "type": [
          "array",
          "null"
        ],
        "items": {
          "type": "string"
        }
      }
    },
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Empty",
    "type": "object"
  }
}
```

</details>

### `dev.leon.terminal.report_status`

*experimental*. Set the block's root program-status record as if its program had sent the OSC 7501 report (`leon notify`), for tools that can't write one to the terminal. The spec's limits apply; text OSC 7501 would drop (controls, line breaks, invisible characters) is a 400.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `state` | string | yes | `working`, `blocked`, `done` or `error` (also `idle`, or `clear` to remove the record). |
| `title` | string \| null |  | One line of text, at most 192 bytes. |
| `msg` | string \| null |  | One line of text, at most 2048 bytes. |

**Output:** none

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "ReportStatus",
    "type": "object",
    "properties": {
      "state": {
        "description": "`working`, `blocked`, `done` or `error` (also `idle`, or `clear` to remove the record).",
        "type": "string"
      },
      "title": {
        "description": "One line of text, at most 192 bytes.",
        "type": [
          "string",
          "null"
        ]
      },
      "msg": {
        "description": "One line of text, at most 2048 bytes.",
        "type": [
          "string",
          "null"
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "state"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Empty",
    "type": "object"
  }
}
```

</details>

### `dev.leon.terminal.program_status`

*stable*. The OSC 7501 program-status records.

**Input:** none

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `records` | [any] | yes | `{id, record}` per record, least recently reported first; `id` null is the root record. A record has `state` and optional `kind`, `progress`, `app` (inherited), `title`, `msg`. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "NoArgs",
    "type": "object",
    "additionalProperties": false
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "ProgramStatus",
    "type": "object",
    "properties": {
      "records": {
        "description": "`{id, record}` per record, least recently reported first; `id` null is the root record.\nA record has `state` and optional `kind`, `progress`, `app` (inherited), `title`, `msg`.",
        "type": "array",
        "items": true
      }
    },
    "required": [
      "records"
    ]
  }
}
```

</details>

### `dev.leon.terminal.lines`

*experimental*. Rows by absolute line number, from the scrollback and the screen, as `block.watch` sends them; lines no longer kept are left out, and so are the last ones past 4 MiB (ask again for them). With the numbering they were read under.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `from` | integer (uint) | yes | The first line's absolute number (the screen's top row is `scrolled`). |
| `count` | integer (uint) | yes | How many lines, at most 1000. |
| `palette` | `Palette` \| null |  | The colours to resolve them with, as `block.watch` takes them [default: leon-dark's]. |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `scrolled` | integer (uint) | yes |  |
| `history` | integer (uint) | yes |  |
| `rows` | [`Row`] | yes |  |

**`Palette`:** A client's terminal colours, as 0xRRGGBB numbers: the default text and background, the cursor and ANSI 0–15.

| Field | Type | Required | Description |
|---|---|---|---|
| `fg` | integer (uint32) | yes |  |
| `bg` | integer (uint32) | yes |  |
| `cursor` | integer (uint32) | yes |  |
| `ansi` | [integer (uint32)] | yes |  |

**`Row`:** One row of a screen update: its spans and its backgrounds (without the line).

| Field | Type | Required | Description |
|---|---|---|---|
| `line` | integer (uint) | yes |  |
| `spans` | [`Span`] | yes |  |
| `backgrounds` | [`Bg`] | yes |  |

**`Span`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `col` | integer (uint) | yes |  |
| `cells` | integer (uint) | yes | Columns covered (a wide char counts two, combining marks none). |
| `text` | string | yes |  |
| `fg` | integer (uint32) | yes |  |
| `bold` | boolean | yes |  |
| `italic` | boolean | yes |  |
| `underline` | boolean | yes |  |
| `strikeout` | boolean | yes |  |

**`Bg`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `col` | integer (uint) | yes |  |
| `len` | integer (uint) | yes |  |
| `color` | integer (uint32) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "LinesArgs",
    "type": "object",
    "properties": {
      "from": {
        "description": "The first line's absolute number (the screen's top row is `scrolled`).",
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "count": {
        "description": "How many lines, at most 1000.",
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "palette": {
        "description": "The colours to resolve them with, as `block.watch` takes them [default: leon-dark's].",
        "anyOf": [
          {
            "$ref": "#/$defs/Palette"
          },
          {
            "type": "null"
          }
        ]
      }
    },
    "additionalProperties": false,
    "required": [
      "from",
      "count"
    ],
    "$defs": {
      "Palette": {
        "description": "A client's terminal colours, as 0xRRGGBB numbers: the default text and background, the cursor\nand ANSI 0–15.",
        "type": "object",
        "properties": {
          "fg": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          },
          "bg": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          },
          "cursor": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          },
          "ansi": {
            "type": "array",
            "items": {
              "type": "integer",
              "format": "uint32",
              "minimum": 0
            },
            "minItems": 16,
            "maxItems": 16
          }
        },
        "additionalProperties": false,
        "required": [
          "fg",
          "bg",
          "cursor",
          "ansi"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Lines",
    "description": "History rows by absolute number (`Terminal::lines`), with the numbering they were read under.",
    "type": "object",
    "properties": {
      "scrolled": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "history": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "rows": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/Row"
        }
      }
    },
    "required": [
      "scrolled",
      "history",
      "rows"
    ],
    "$defs": {
      "Row": {
        "description": "One row of a screen update: its spans and its backgrounds (without the line).",
        "type": "object",
        "properties": {
          "line": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "spans": {
            "type": "array",
            "items": {
              "$ref": "#/$defs/Span"
            }
          },
          "backgrounds": {
            "type": "array",
            "items": {
              "$ref": "#/$defs/Bg"
            }
          }
        },
        "required": [
          "line",
          "spans",
          "backgrounds"
        ]
      },
      "Span": {
        "type": "object",
        "properties": {
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "cells": {
            "description": "Columns covered (a wide char counts two, combining marks none).",
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "text": {
            "type": "string"
          },
          "fg": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          },
          "bold": {
            "type": "boolean"
          },
          "italic": {
            "type": "boolean"
          },
          "underline": {
            "type": "boolean"
          },
          "strikeout": {
            "type": "boolean"
          }
        },
        "required": [
          "col",
          "cells",
          "text",
          "fg",
          "bold",
          "italic",
          "underline",
          "strikeout"
        ]
      },
      "Bg": {
        "type": "object",
        "properties": {
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "len": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "color": {
            "type": "integer",
            "format": "uint32",
            "minimum": 0
          }
        },
        "required": [
          "col",
          "len",
          "color"
        ]
      }
    }
  }
}
```

</details>

### `dev.leon.terminal.find`

*experimental*. Every match of a query in the scrollback and the screen, oldest first; a match may cross a wrapped line. A bad regex is a 400.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `query` | string | yes |  |
| `regex` | boolean |  | The query is a regular expression (Rust `regex` syntax); else it is plain text. |
| `case_sensitive` | boolean |  |  |
| `limit` | integer (uint) |  | At most this many matches (and at most `MAX_MATCHES`). |

**Output:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `matches` | [`Range`] | yes | Oldest first. |
| `truncated` | boolean | yes | More matches exist than were returned. |

**`Range`:** From `start` to `end`, both cells included, in reading order.

| Field | Type | Required | Description |
|---|---|---|---|
| `start` | `Pos` | yes |  |
| `end` | `Pos` | yes |  |

**`Pos`:** A cell: its absolute line and its column.

| Field | Type | Required | Description |
|---|---|---|---|
| `line` | integer (uint) | yes |  |
| `col` | integer (uint) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Query",
    "type": "object",
    "properties": {
      "query": {
        "type": "string"
      },
      "regex": {
        "description": "The query is a regular expression (Rust `regex` syntax); else it is plain text.",
        "type": "boolean",
        "default": false
      },
      "case_sensitive": {
        "type": "boolean",
        "default": false
      },
      "limit": {
        "description": "At most this many matches (and at most `MAX_MATCHES`).",
        "type": "integer",
        "format": "uint",
        "minimum": 0,
        "default": 10000
      }
    },
    "additionalProperties": false,
    "required": [
      "query"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Found",
    "type": "object",
    "properties": {
      "matches": {
        "description": "Oldest first.",
        "type": "array",
        "items": {
          "$ref": "#/$defs/Range"
        }
      },
      "truncated": {
        "description": "More matches exist than were returned.",
        "type": "boolean"
      }
    },
    "required": [
      "matches",
      "truncated"
    ],
    "$defs": {
      "Range": {
        "description": "From `start` to `end`, both cells included, in reading order.",
        "type": "object",
        "properties": {
          "start": {
            "$ref": "#/$defs/Pos"
          },
          "end": {
            "$ref": "#/$defs/Pos"
          }
        },
        "required": [
          "start",
          "end"
        ]
      },
      "Pos": {
        "description": "A cell: its absolute line and its column.",
        "type": "object",
        "properties": {
          "line": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          }
        },
        "additionalProperties": false,
        "required": [
          "line",
          "col"
        ]
      }
    }
  }
}
```

</details>

### `dev.leon.terminal.selection`

*experimental*. A selection's range (words, lines across wraps, a rectangle; half a wide character takes the whole one) and, if asked, the text it copies; null when `anchor` isn't kept.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `kind` | `Kind` | yes |  |
| `anchor` | `Pos` | yes | Where the selection started and where it reaches now, by absolute line. |
| `head` | `Pos` | yes |  |
| `text` | boolean |  | Also return the text it copies. |

**Output:** `Selected` | null

**`Kind`:** `"simple"` | `"word"` | `"line"` | `"block"`.

**`Pos`:** A cell: its absolute line and its column.

| Field | Type | Required | Description |
|---|---|---|---|
| `line` | integer (uint) | yes |  |
| `col` | integer (uint) | yes |  |

**`Selected`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `start` | `Pos` | yes |  |
| `end` | `Pos` | yes |  |
| `text` | string \| null |  | The text it copies, when asked for: no newline inside a wrapped line. |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "SelectionArgs",
    "type": "object",
    "properties": {
      "kind": {
        "$ref": "#/$defs/Kind"
      },
      "anchor": {
        "description": "Where the selection started and where it reaches now, by absolute line.",
        "$ref": "#/$defs/Pos"
      },
      "head": {
        "$ref": "#/$defs/Pos"
      },
      "text": {
        "description": "Also return the text it copies.",
        "type": "boolean",
        "default": false
      }
    },
    "additionalProperties": false,
    "required": [
      "kind",
      "anchor",
      "head"
    ],
    "$defs": {
      "Kind": {
        "oneOf": [
          {
            "description": "From one cell to the other, in reading order.",
            "type": "string",
            "const": "simple"
          },
          {
            "description": "Whole words (a double-click).",
            "type": "string",
            "const": "word"
          },
          {
            "description": "Whole lines, a wrapped line as one (a triple-click).",
            "type": "string",
            "const": "line"
          },
          {
            "description": "The rectangle with the two cells at its corners.",
            "type": "string",
            "const": "block"
          }
        ]
      },
      "Pos": {
        "description": "A cell: its absolute line and its column.",
        "type": "object",
        "properties": {
          "line": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          }
        },
        "additionalProperties": false,
        "required": [
          "line",
          "col"
        ]
      }
    }
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Nullable_Selected",
    "anyOf": [
      {
        "$ref": "#/$defs/Selected"
      },
      {
        "type": "null"
      }
    ],
    "$defs": {
      "Selected": {
        "type": "object",
        "properties": {
          "start": {
            "$ref": "#/$defs/Pos"
          },
          "end": {
            "$ref": "#/$defs/Pos"
          },
          "text": {
            "description": "The text it copies, when asked for: no newline inside a wrapped line.",
            "type": [
              "string",
              "null"
            ]
          }
        },
        "required": [
          "start",
          "end"
        ]
      },
      "Pos": {
        "description": "A cell: its absolute line and its column.",
        "type": "object",
        "properties": {
          "line": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          }
        },
        "additionalProperties": false,
        "required": [
          "line",
          "col"
        ]
      }
    }
  }
}
```

</details>

### `dev.leon.terminal.link_at`

*experimental*. The link on a cell: an OSC 8 hyperlink, else a URL in its line (across wraps); null if none.

**Input:** 

| Field | Type | Required | Description |
|---|---|---|---|
| `line` | integer (uint) | yes |  |
| `col` | integer (uint) | yes |  |

**Output:** `Link` | null

**`Link`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `uri` | string | yes |  |
| `start` | `Pos` | yes |  |
| `end` | `Pos` | yes |  |

**`Pos`:** A cell: its absolute line and its column.

| Field | Type | Required | Description |
|---|---|---|---|
| `line` | integer (uint) | yes |  |
| `col` | integer (uint) | yes |  |

<details><summary>JSON Schemas</summary>

```json
{
  "input_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Pos",
    "description": "A cell: its absolute line and its column.",
    "type": "object",
    "properties": {
      "line": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      },
      "col": {
        "type": "integer",
        "format": "uint",
        "minimum": 0
      }
    },
    "additionalProperties": false,
    "required": [
      "line",
      "col"
    ]
  },
  "output_schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "Nullable_Link",
    "anyOf": [
      {
        "$ref": "#/$defs/Link"
      },
      {
        "type": "null"
      }
    ],
    "$defs": {
      "Link": {
        "type": "object",
        "properties": {
          "uri": {
            "type": "string"
          },
          "start": {
            "$ref": "#/$defs/Pos"
          },
          "end": {
            "$ref": "#/$defs/Pos"
          }
        },
        "required": [
          "uri",
          "start",
          "end"
        ]
      },
      "Pos": {
        "description": "A cell: its absolute line and its column.",
        "type": "object",
        "properties": {
          "line": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          },
          "col": {
            "type": "integer",
            "format": "uint",
            "minimum": 0
          }
        },
        "additionalProperties": false,
        "required": [
          "line",
          "col"
        ]
      }
    }
  }
}
```

</details>
