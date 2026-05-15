# Changelog

All notable changes to **beegui** are tracked here. The
format follows [Keep a Changelog]; the project adheres to
[Semantic Versioning].

[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html

## [Unreleased]

TBD.

## [0.4.0] - 2026-05-15

The "operational completeness" release. v0.3 reached visual parity
with bee-tui; v0.4 closes the operational gaps: webhook firing,
native desktop notifications, GSOC subscriptions, pubsub history
files, theme presets, and prebuilt installers for five platforms
via cargo-dist.

### Added

- **Alerts webhook firing.** Gate transitions surfaced by
  `AlertsPipeline` now POST to `[alerts] webhook_url` via
  `bee_cockpit_core::alerts::fire`. Slack/Discord-compatible
  body, 10-second timeout, single warn-log on failure.
- **Native desktop notifications.** `[notifications] desktop =
  true` raises an OS-level notification on every Fail/Warn
  transition (libnotify on Linux via zbus, Notification Center
  on macOS, toast on Windows — all through `notify-rust`).
- **GSOC subscribe.** S14 Pubsub now toggles between PSS
  (topic-only) and GSOC (owner + identifier). Same view layer,
  same ring buffer.
- **Pubsub history file.** Optional path field in S14; when
  populated, every received message is appended as JSONL via
  `bee_cockpit_core::pubsub::open_history_writer` with 64 MiB
  rotation + 5-file retention. Replayable by any tool that
  understands the same format.
- **Theme presets.** `--theme {auto,light,dark}` CLI flag or
  `[ui] theme = "..."` config. `auto` follows egui's
  OS-derived default; `light` / `dark` lock the visual scheme.
- **cargo-dist installers.** `dist-workspace.toml` + Release
  workflow build prebuilt binaries for darwin (x86_64 +
  aarch64), linux (x86_64 + aarch64), and windows (x86_64) on
  every tag push; shell + powershell one-liner installers fetch
  from the GitHub release.
- **Alerts panel diagnostics.** Status chips at the top of the
  popup show whether webhook firing and desktop notifications
  are active for the current config.

### Notes

- Operational parity with bee-tui modulo command-bar / `:verb`
  interactive flows. The CLI `--once` surface and the GUI
  screens cover the rest.
- Release profile tuned for installer size (`lto = "thin"` in
  `[profile.dist]`, `opt-level = "s"` + `strip = true` in
  `[profile.release]`).

## [0.3.0] - 2026-05-15

The "feature parity" release. Every placeholder screen now ships a
working worker, the bottom log pane streams `bee::http` traffic,
gate transitions surface as in-app alerts, and the full bee-tui
`--once` verb surface is reachable via CLI.

### Added

- **S11 Manifest** — paste a root reference + click *Load*.
  `bee_cockpit_core::manifest_walker::load_node` fetches the root
  chunk; clicking each `▶` lazily loads that fork's child node.
- **S12 Watchlist** — add references one at a time + *Re-check
  all* button. Each ref runs through
  `bee_cockpit_core::durability::check`; results land in a
  rolling 50-entry history rendered via
  `views::watchlist::view_for`.
- **S13 Feed Timeline** — owner + topic + max-entries inputs;
  *Walk feed* spawns `feed_timeline::walk`; results render via
  `views::feed_timeline::view_for`.
- **S14 Pubsub** — topic input + *Subscribe (PSS)* button.
  Spawns `pubsub::spawn_pss_watcher` with a cancellation token;
  messages tail into a 200-entry ring with optional case-
  insensitive substring filter.
- **S15 Fleet** — when launched with multiple node URLs
  (positional CLI or `[[nodes]]` in config), `fleet::spawn_poller`
  is spawned; the screen renders the aggregate roll-up via
  `views::fleet::view_for`. Single-node mode shows a nudge.
- **Log pane.** Bottom panel toggleable via the status-bar
  *Logs* button or **Ctrl+L**. Renders the same `bee::http`
  events that drive S8 API Health's call-stats. Both pane and
  S8 share one `LogCapture`.
- **Alerts pipeline.** Each frame the App runs
  `views::health::gates_for_with_stamps` and feeds the result
  to `alerts::AlertState::diff_and_record`. Surfaced transitions
  land in a 100-entry ring and appear in a popup (status-bar
  *🔔 Alerts* button or **Ctrl+A**) with from→to, value, why,
  age. Webhook firing is parity-pending — coming with a
  `[alerts] webhook_url` config knob.
- **`--once` CLI** — full bee-tui verb surface. `--once
  readiness`, `hash <path>`, `cid <ref>`, `depth-table`,
  `pss-target`, `gsoc-mine`, `version-check`,
  `config-doctor`, `price`, `basefee`, `inspect`,
  `durability-check`, `upload-file`, `upload-collection`,
  `feed-probe`, `feed-timeline`, `grantees-list`,
  `buy-preview`, `buy-suggest`, `topup-preview`,
  `dilute-preview`, `extend-preview`, `plan-batch`. Pair with
  `--json` for CI-friendly output.
- **Logging.** beegui installs the `bee_cockpit_core::log_capture`
  tracing layer at startup so the bottom pane and S8 are
  populated.

### Notes

- 15/15 screens now render live data. The placeholder strip in
  v0.2 is gone.
- Theme/keybinding configuration and prebuilt installers (via
  cargo-dist) remain on the roadmap for v0.4.

## [0.2.0] - 2026-05-15

First feature release. beegui ships a working egui-based desktop
cockpit consuming
[`bee-cockpit-core`](https://crates.io/crates/bee-cockpit-core) `0.1`
— the same logic crate that powers bee-tui.

### Added

- **15 screens** reachable via the tab bar or keys `1`–`9` /
  `Tab` / `Shift+Tab`. 10 stream live data through
  `BeeWatch`:
  - S1 Health — gate ladder via
    `views::health::gates_for_with_stamps`.
  - S2 Stamps — postage batch table via `views::stamps::rows_for`.
  - S3 Swap — chequebook card + cheques + settlements via
    `views::swap::view_for_no_market`.
  - S4 Lottery — round phase ribbon + anchors + stake card via
    `views::lottery::view_for`.
  - S5 Warmup — bootstrap checklist with progress bars +
    elapsed counter via `views::warmup::view_for`.
  - S6 Peers — bin strip + peer table via
    `views::peers::view_for`.
  - S7 Network — identity, AutoNAT reachability, underlays via
    `views::network::view_for`.
  - S8 API Health — chain state + pending transactions via
    `views::api_health::view_for`.
  - S9 Tags — progress + counts via `views::tags::view_for`.
  - S10 Pins — list view via `views::pins::view_for`.
- **CLI surface** matching bee-tui: `--node`, `--token`,
  `--config`, positional URLs. Environment overrides:
  `BEE_NODE_URL`, `BEE_NODE_TOKEN`, `BEEGUI_CONFIG`,
  `BEEGUI_DATA`.
- **Config file** support via core's `load_raw<Config>`. Same
  TOML schema as bee-tui (minus TUI-only `[keybindings]` /
  `[styles]`).
- **Status bar** with connection dot, active node URL, and
  beegui version.

### Deferred to v0.3

- **S11 Manifest** — needs root-reference input + the walker
  wired up.
- **S12 Watchlist** — needs the durability worker.
- **S13 Feed Timeline** — needs owner+topic input + the walker.
- **S14 Pubsub** — needs a subscription worker.
- **S15 Fleet** — needs the multi-node poller + aggregator.
- **Log pane** — Bee process logs streamed into a bottom pane
  (also unblocks S8's HTTP call-stats).
- **Alerts pipeline** + tray notifications.
- **Command bar** for interactive verbs (uploads, batch buys,
  manifest walks, etc.).
- **Theme presets**.

### Notes

beegui depends on `bee-cockpit-core = "0.1"` and `bee-rs = "1.6"`.
The core dep is the only reason the binary is non-trivial — every
view computation comes from there, byte-identical to bee-tui's
output.
