# beegui

Desktop GUI cockpit for [Ethereum Swarm](https://www.ethswarm.org/)
Bee node operators. Sibling of
[`bee-tui`](https://github.com/ethswarm-tools/bee-tui) — same cockpit
logic (health gates, stamp warnings, fleet roll-up, redistribution
skip reasons, …) via the shared
[`bee-cockpit-core`](https://github.com/ethswarm-tools/bee-cockpit-core)
crate, rendered with [egui](https://github.com/emilk/egui) instead of
ratatui.

## Why egui

Same Rust, single static binary, no JavaScript / no Electron — matches
bee-tui's "no runtime, no Docker" promise. egui's immediate-mode model
maps cleanly to bee-cockpit-core's `view_for(snapshot) -> View` pattern
so the porting effort sits in the widget layer, not in re-implementing
cockpit logic.

## Status

**0.9.0** — external Bee log tailing. `--bee-log <path>` /
`--bee-log-cmd <cmd>` (or `[bee] log_file` / `log_command` in
config) pipe Bee's own log output into a new tabbed bottom pane
alongside the existing `bee::http` tail; auto-discovery on Linux
walks `/proc` for the Bee process behind the active node URL.
Earlier capabilities — switch active node (v0.8), `rchash` bench
(v0.8), navigation parity (v0.7), palette verbs (v0.5/v0.6),
webhook + desktop notifications, GSOC + pubsub history, theme
presets, cargo-dist installers, full `--once` surface — remain.

| Screen | State |
|---|---|
| S1 Health | gates + Stamp TTL |
| S2 Stamps | table + status + bucket-histogram drill |
| S3 Swap | chequebook + cheques + settlements |
| S4 Lottery | round + anchors + stake + `rchash` bench |
| S5 Warmup | checklist + elapsed + progress bars |
| S6 Peers | bin strip + peer table + drill |
| S7 Network | identity + reachability + underlays |
| S8 API Health | chain + pending tx + HTTP call-stats |
| S9 Tags | progress + counts |
| S10 Pins | list + check pipeline + sort |
| S11 Manifest | paste root ref → lazy fork walker |
| S12 Watchlist | per-ref durability worker + history |
| S13 Feed Timeline | owner+topic walker |
| S14 Pubsub | PSS subscriber + ring buffer + filter |
| S15 Fleet | multi-node poller + roll-up + switch active |

## Install

Prebuilt installers (no Rust toolchain required):

```sh
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ethswarm-tools/beegui/releases/latest/download/beegui-installer.sh | sh

# Windows
powershell -c "irm https://github.com/ethswarm-tools/beegui/releases/latest/download/beegui-installer.ps1 | iex"
```

Or via cargo:

```sh
cargo install beegui
```

## Usage

```sh
beegui                            # connect to http://localhost:1633
beegui --node http://host:1633    # explicit node URL
beegui --token <bearer>           # restricted-mode auth
beegui --config ~/beegui.toml     # explicit config file
beegui --theme light              # auto | light | dark
beegui http://a:1633 http://b:1633  # positional URLs (ad-hoc fleet)
beegui --bee-log /var/log/bee.log   # tail a Bee log file
beegui --bee-log-cmd "journalctl -u bee -f"   # …or a command's stdout
```

Environment overrides: `BEE_NODE_URL`, `BEE_NODE_TOKEN`,
`BEEGUI_CONFIG`, `BEEGUI_DATA`.

### Config file

beegui reads the same TOML schema as bee-tui (minus the TUI-only
`[keybindings]` / `[styles]` sections). Drop a file at
`~/.config/beegui/config.toml`:

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

[alerts]
webhook_url = "https://hooks.slack.com/services/…"
debounce_secs = 300

[notifications]
desktop = true        # libnotify / macOS notif center / Windows toast

[ui]
theme = "auto"        # auto | light | dark
```

### Keys

| Key | Action |
|---|---|
| `1`–`9` | Jump to that screen |
| `Tab` / `Shift+Tab` | Cycle screens |
| `:` or `Ctrl+P` | Open command palette |
| `?` | Open the help overlay |
| `Ctrl+L` | Toggle the bottom log pane |
| `Ctrl+A` | Toggle the alerts popup |
| `↑/↓` or `j/k` | Move selection in the active list |
| `Enter` / click | Drill into the selected row |
| `PgUp` / `PgDn` | Page selection ±10 rows |
| `Home` / `End` | First / last row |
| `r` | Re-poll fleet (S15) · run `rchash` bench (S4) |
| `c` | Check all pins (S10) |
| `s` | Cycle pin sort mode (S10) |
| `Esc` | Close any overlay or drill |
| Click a tab | Same as `1`–`9` |

### `--once` verbs (no GUI)

```sh
beegui --once readiness --json http://localhost:1633
beegui --once depth-table
beegui --once hash ./somefile
beegui --once durability-check <ref>
beegui --once buy-preview --json
```

Full verb list:
`hash`, `cid`, `depth-table`, `pss-target`, `gsoc-mine`,
`readiness`, `version-check`, `check-version`,
`config-doctor`, `price`, `basefee`, `inspect`,
`durability-check`, `upload-file`, `upload-collection`,
`feed-probe`, `feed-timeline`, `grantees-list`,
`buy-preview`, `buy-suggest`, `topup-preview`,
`dilute-preview`, `extend-preview`, `plan-batch`.

## Screenshots

Visual assets aren't committed yet. The capture recipe lives at
[`docs/screenshots/`](docs/screenshots/) — Linux Wayland / X11,
macOS, and Windows commands plus framing tips and the asset
inventory the README will embed once captured.

## Building from source

```sh
git clone https://github.com/ethswarm-tools/beegui
cd beegui
cargo run --release
```

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE)
or [MIT license](./LICENSE-MIT) at your option.
