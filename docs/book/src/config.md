# Configuration

beegui reads the same TOML schema as bee-tui — minus the TUI-only
`[keybindings]` and `[styles]` sections. The default search path
is `~/.config/beegui/config.toml` on Linux, the platform
equivalents on macOS and Windows. Override with `--config <path>`
or `BEEGUI_CONFIG=<path>`.

## Full example

```toml
[[nodes]]
name = "local"
url = "http://localhost:1633"
default = true

[[nodes]]
name = "remote"
url = "http://bee.example.com:1633"
token = "@env:BEE_TOKEN"
log_command = "ssh bee-host 'tail -f /var/log/bee.log'"

[[nodes]]
name = "supervised"
url = "http://localhost:1733"
log_file = "/var/log/bee/bee.log"

[bee]
bin = "/usr/local/bin/bee"
config = "/etc/bee/config.yaml"

[bee.logs]
rotate_size_mb = 64       # rotate when reaching this size
keep_files = 5            # how many rotated files to retain

[alerts]
webhook_url = "https://hooks.slack.com/services/…"
debounce_secs = 300

[notifications]
desktop = true            # libnotify / macOS notif center / Windows toast

[pubsub]
history_file = "~/pubsub.jsonl"
rotate_size_mb = 64
keep_files = 5

[ui]
theme = "auto"            # auto | light | dark
```

## Sections

### `[[nodes]]`

One block per node. Required: `name`, `url`. Optional: `token`,
`default`, `log_file`, `log_command`.

- `default = true` marks the active node at startup. If no node
  sets it, the first one wins.
- `token` accepts `"@env:VAR"` to read from an environment
  variable.
- `log_file` and `log_command` are per-node bee-log sources; see
  [Bee log tailing](./bee-log.md) for the priority order.

### `[bee]`

When set, beegui spawns Bee as a child process at startup. Both
`bin` and `config` are required; partial config is a hard error.
See [Bee process supervision](./supervisor.md).

### `[bee.logs]`

Log rotation knobs for the supervised Bee's stdout+stderr capture
file. Defaults: 64 MiB × 5 files (≈320 MiB ceiling).

### `[alerts]`

`webhook_url` fires gate-transition alerts at a Slack-compatible
webhook. `debounce_secs` suppresses re-firing a gate within that
window (default 60s; 300 is more humane for chat channels).

### `[notifications]`

`desktop = true` raises OS-native notifications on Fail / Warn
gate transitions. Linux uses libnotify (zbus), macOS uses the
Notification Center, Windows uses toasts — all via `notify-rust`.

### `[pubsub]`

Optional persistence of S14 pubsub messages to a JSONL file.
Rotates on size; older rotations get unlinked. Replayable by any
tool that understands the same format (bee-tui has
`:pubsub-replay`; beegui's replay is on the roadmap).

### `[ui]`

`theme` accepts `auto` / `light` / `dark`. CLI `--theme` overrides.
