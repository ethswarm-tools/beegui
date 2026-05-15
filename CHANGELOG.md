# Changelog

All notable changes to **beegui** are tracked here. The
format follows [Keep a Changelog]; the project adheres to
[Semantic Versioning].

[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html

## [Unreleased]

TBD.

## [0.11.0] - 2026-05-16

The "bee supervision" release. beegui catches up to bee-tui's
oldest non-parity item: spawning + owning the Bee process for
the session instead of connecting to one the operator started
separately.

### Added

- **`--bee-bin <PATH>` + `--bee-config <PATH>` CLI flags.**
  When both are set, beegui forks Bee as a child process,
  redirects stdout+stderr to a rotating temp log, waits up to
  60s for `/health` to return 200, then opens the cockpit.
- **`[bee]` config block.** `bin` / `config` keys mirror the CLI
  flags for persistent configuration; `[bee.logs]` controls the
  rotating writer (default 64 MiB × 5 files). Partial config
  (`bin` without `config`, or vice-versa) is a hard error so a
  typo can't silently skip the spawn.
- **Supervisor status chip** in the bottom status bar:
  `● bee running` / `✕ bee exited (code N)` / `✕ bee killed
  (signal N)`. Polled every frame; non-running states surface
  in red so an OOM kill or crash is immediately visible without
  opening the log pane.
- **Auto-routed Bee logs.** When the supervisor is active its
  rotating log file becomes the bee-log source automatically —
  the bottom pane's Errors / Warn / Info / Debug / Bee HTTP
  tabs are populated without any `--bee-log` flag. Source
  origin shows as `[supervised]` in the pane header.
- **Clean shutdown on quit.** `on_exit` SIGTERMs the process
  group; 5-second grace; escalates to SIGKILL if Bee hasn't
  exited. Bee's clean shutdown closes RocksDB cleanly so
  recovery isn't needed on next start.

### Notes

- Auto-restart watchdog (bee-tui's `[bee.supervisor]
  auto_restart = true`) is deferred — v0.11 always behaves as
  if `auto_restart = false`: log the exit, dim the status chip,
  no respawn. Operators restart beegui to retry.
- When the supervisor is on, the v0.9 `--bee-log` /
  `--bee-log-cmd` flags + auto-discovery are ignored for the
  active node (the supervisor's log file is the freshest
  source).

## [0.10.0] - 2026-05-15

The "switch from anywhere" release. v0.8 made it possible to
switch the active node from S15 Fleet by pressing Enter on a row;
v0.10 makes that flow available from every screen, mirroring
bee-tui v1.10's `Ctrl+N` + `:context`.

### Added

- **`Ctrl+N` node picker.** Opens a centred popup listing every
  `[[nodes]]` entry (or every positional CLI URL). `↑/↓` (or
  `j/k`) navigate; `Enter` switches; click also works; `Esc`
  cancels. A green dot marks the currently active node; the
  picker pre-selects it so `Ctrl+N Enter` is a no-op safety
  net.
- **`:nodes` / `:node` palette verb** — same as `Ctrl+N`. Lets
  operators who live in the palette open the picker without
  reaching for the modifier.
- **`:context <name>` palette verb** (alias `:ctx`, `:switch`)
  — typed switch by node name. Skips the picker entirely;
  banner reports the result.

### Notes

- Switching reuses the v0.8 `switch_active_node` plumbing — the
  BeeWatch hub is torn down + rebuilt against the new endpoint
  and the v0.9 bee-log tailer re-resolves discovery against the
  new node. The fleet poller (the data backing S15) keeps
  running across switches.

## [0.9.0] - 2026-05-15

The "external Bee logs" release. bee-tui has been tailing the Bee
process's own log output since v1.15; beegui catches up by reusing
the same `bee-cockpit-core` plumbing.

### Added

- **`--bee-log <PATH>` CLI flag.** Tail a Bee log file directly.
  Starts at EOF so pre-existing history doesn't replay; survives
  log rotation (`logrotate`-style inode swaps) and truncation.
- **`--bee-log-cmd <CMD>` CLI flag.** Tail a shell command's
  stdout — `journalctl -u bee -f`, `docker logs -f bee`,
  `ssh host 'tail -f /var/log/bee.log'` — for nodes whose log
  isn't a plain file the cockpit can read.
- **`[bee] log_file` / `log_command` config.** Per-node defaults
  (same TOML schema as bee-tui's `[[nodes]]`); CLI flags
  override.
- **Auto-discovery (Linux, local nodes).** When no explicit
  source is configured, beegui walks `/proc` to find the Bee
  process behind the active node URL and picks the right
  source: regular file → tail directly; pipe under docker →
  `docker logs -f`; pipe under systemd → `journalctl -u …`.
  Non-Linux hosts and remote URLs fall through to the
  no-source placeholder.
- **Tabbed log pane.** The bottom log pane (Ctrl+L) now has 7
  tabs matching bee-tui's: Errors / Warn / Info / Debug /
  Bee HTTP (from the Bee tailer) plus the existing bee::http
  tab (the cockpit's outbound calls — was the only tab in
  v0.8) and a Cockpit tab surfacing the cockpit's own
  tracing events. Tab entry counts surface in the strip.
- **Source label.** The pane header shows the resolved source
  + its origin (`CLI` / `config` / `discovered`) so operators
  understand whether the Bee-side tabs are empty for a
  configurable reason.
- **Switches re-resolve.** Changing the active node from S15
  Fleet kills the old tailer and re-resolves discovery /
  config against the new node, so the Bee-side tabs always
  reflect the currently selected node's process.

### Internal

- New `src/bee_log.rs` module owns the per-tab ring buffers
  (1000 lines/tab) and the source-resolution priority chain
  (CLI > config > discovery). The tailer itself lives in
  `bee_cockpit_core::bee_log_tailer` — beegui only adds the
  egui rendering + the CLI plumbing.

## [0.8.1] - 2026-05-15

Documentation patch. The in-app help overlay and README "Keys"
table had drifted: v0.7 and v0.8 keybindings landed in the code
but weren't surfaced in the documentation operators see.

### Fixed

- **In-app help overlay** (`?`) — added rows for `↑ ↓` / `j k`,
  `Enter` / click, `PgUp` / `PgDn`, `Home` / `End`, `r`
  (fleet re-poll · lottery `rchash` bench), `c` (pins check
  all), `s` (pin sort cycle). Previously the overlay listed
  only the 7 v0.5-era keys.
- **README "Keys" table** — same backfill so the GitHub
  landing page matches the in-app overlay.

### Added

- **`docs/screenshots/`** scaffold with a capture recipe
  (Linux Wayland / X11, macOS, Windows), framing tips, and
  asset inventory. The README now references image placeholders
  (`cold-start.png`, `s2-stamps.png`, `s6-peers.png`,
  `s15-fleet.png`, `palette.png`); captures land in a future
  commit.

## [0.8.0] - 2026-05-15

The "switch active node" release. v0.7.1 left switching from
Fleet on the backlog because tearing down the BeeWatch hub for a
different endpoint was a bigger refactor; v0.8 ships it. Also:
S4 Lottery gets bee-tui's `rchash` benchmark.

### Added

- **S15 Fleet — switch active node.** Pressing `Enter` (or
  double-clicking a row, or hitting the *Switch to <name>*
  button) on a non-active node tears down the current
  `BeeWatch`, builds a fresh `ApiClient` for the target, and
  spawns a new watch hub. The shared fleet poller keeps running
  so the roll-up view doesn't blink. Alerts pipeline is recreated
  on switch so the first frame on the new node doesn't fire
  spurious "Unknown → X" transitions. Mirrors bee-tui's
  `Enter → SwitchContext` flow.
- **S4 Lottery — `rchash` benchmark.** Pressing `r` (or
  clicking *Run*) times the redistribution-sample lookup at the
  health-derived storage depth against the full anchor range,
  same as bee-tui's S4. Verdict colors green under 95s and red
  above (the reveal-phase deadline).

### Internal

- App keeps two independent `CancellationToken`s: `cancel` for
  app shutdown and `watch_cancel` for the currently active
  node, so switching tears down only the watch hub.
- Dead-code cleanup: dropped the unused `BannerLevel::Warn`
  variant, the `PaletteOutcome` wrapper enum (palette banners
  now flow directly through the mpsc channel), and
  `OnceResult::ok`.

## [0.7.1] - 2026-05-15

Bug fix release. Audit after v0.7 surfaced four more interaction
gaps vs bee-tui; all four are addressed here.

### Fixed

- **S10 Pins** — `Enter` (or click) now runs an integrity check
  on the highlighted pin; `c` checks all pins; `s` cycles the
  sort mode (Reference → BadFirst → TotalChunks). Previously
  every pin sat at "unchecked" forever because the check
  pipeline wasn't wired up.
- **S11 Manifest** — `↑/↓/j/k` navigates the tree; `Enter`
  expands/collapses the highlighted fork. Previously only
  mouse click worked.
- **S15 Fleet** — `r` re-polls every node (kicks the
  `spawn_poller` resync channel). New *Re-poll all* button in
  the screen header does the same.
- **Paging** — `PageUp` / `PageDown` (±10) and `Home` / `End`
  added to every list-based screen: tags, pins, peers, stamps,
  watchlist, feed-timeline, pubsub, fleet, manifest.

### Notes

- Switching the *active* node from Fleet (bee-tui's
  `Enter → SwitchContext`) is still on the v0.8 backlog —
  tearing down + restarting the BeeWatch hub for a different
  endpoint is a bigger refactor than this patch deserves.

## [0.7.0] - 2026-05-15

The "navigation parity" release. Fixes the bug where clicking a
peer didn't do anything; adds keyboard navigation (arrow keys +
j/k) and per-row click handling across every list-based screen.

### Fixed

- **S6 Peers** — clicking a peer (or pressing Enter on the
  selected row) now loads the **PeerDrillFetch** (balance,
  cheques, settlement, ping, status_peers, local_status — six
  parallel `/peers/...` calls) and renders the drill panel with
  every field from bee-tui's S6 drill: balance, ping, settl.
  in/out, last cheques in/out, storage radius, reserve size,
  pullsync rate, batch commitment (with >5% outlier flag). Esc
  closes. Up/Down/j/k navigate the peer list.
- **S2 Stamps** — clicking a row (or Enter) loads
  `get_postage_batch_buckets` for that batch and renders the
  bucket-histogram drill (fill distribution across the six
  bins + top-10 worst buckets by collisions + economics, when
  present). Esc closes. Up/Down/j/k navigate.

### Added

- **Keyboard navigation** across every list-based screen:
  arrow keys + j/k for selection; Enter or click triggers the
  screen's primary action; Esc closes drill panels.
- **Row click** semantics match bee-tui's Enter key for every
  screen — operators with a mouse don't need the keyboard.
  Where bee-tui used a cursor + Enter, beegui uses click =
  select + drill (or click = select, Enter = drill).
- **Pubsub selection** — clicking a message shows its full
  payload preview in a detail pane below the table.
- **Feed Timeline selection** — clicking an entry shows the
  full reference hex (the table column shortens it to a
  prefix).
- **Focus-aware shortcuts.** Global key shortcuts (digit
  screen-jumps, Tab cycling, `?` help, arrow nav) now suppress
  themselves when a text input owns keyboard focus, so typing
  `5` into the feed-timeline owner field no longer jumps to S5.

### Notes

- Selection highlight is a subtle blue band on the row; this
  is the egui equivalent of bee-tui's cursor glyph.
- Stamps and Peers drills run their fetches on the tokio
  runtime and reflect the result on the next frame, identical
  to bee-tui's flow.

## [0.6.0] - 2026-05-15

The "active verbs" release. v0.5 added the palette + inspection
verbs; v0.6 adds the verbs that *do* things — uploads, batch
math, and PSS sends.

### Added

- **`:upload <path> [batch-prefix]`.** Single-shot file or
  directory upload. The batch is auto-picked (longest-TTL usable
  batch) when no prefix is supplied; the returned reference
  surfaces as a banner. Directory uploads use core's
  `uploads::walk_dir` + bee-rs's collection-entries endpoint,
  with the same hidden-file / symlink / size rules as
  `--once upload-collection`.
- **Drag-and-drop.** Dropping a file onto the beegui window
  opens the palette pre-filled with `:upload <path>` so the
  operator just hits Enter to ship.
- **`:batch buy <depth> <amount>` / `:batch topup|dilute|extend
  <batch-prefix> <arg>`.** Stamp-batch math via
  `stamp_preview::buy_preview` / `topup_preview` /
  `dilute_preview` / `extend_preview`. Result lands in the
  banner; this is preview-only (no transaction is sent), same
  as `--once buy-preview` etc.
- **`:feed-probe <owner> <topic>`.** Fetch latest feed update
  via `feed_probe::probe`; banner shows index, payload size,
  reference.
- **`:pss <topic> <payload> [batch-prefix]`.** Sends a PSS
  message. Hex 32-byte topic is parsed as-is; anything else is
  keccak256-of-string (Bee's `Topic::from_string` convention).
  Batch auto-picked when no prefix supplied.

### Notes

- `:batch` math is preview-only. Sending the actual buy /
  topup / dilute / extend transaction stays in `--once`'s lane
  for now because it touches real BZZ and warrants explicit
  confirmation flow design.
- This brings the active palette verb count to 17 — broadly
  in line with bee-tui's interactive `:` set.

## [0.5.0] - 2026-05-15

The "command palette" release. The last interactive parity gap
vs bee-tui — typing verbs to drive the cockpit — closes here.

### Added

- **Command palette.** Open with `:` or `Ctrl+P`. Filter the verb
  list by typing; navigate with `↑/↓`; submit with `Enter`;
  dismiss with `Esc`. Each suggestion shows a one-line summary
  and a usage hint when highlighted.
- **Verbs.** First batch lands with v0.5:
  - `:go <screen>` (also bare `:health`, `:stamps`, etc.) —
    switch screens by name.
  - `:inspect <ref>` / `:manifest <ref>` — jump to S11 and load
    that root reference.
  - `:feed-timeline <owner> <topic> [max]` — jump to S13 and
    walk the feed.
  - `:durability <ref>` — jump to S12, add the reference, run
    the check.
  - `:hash <path>` — compute swarm hash, surface as a banner.
  - `:cid <ref> [manifest|feed]` — compute CID, surface as a
    banner.
  - `:diagnose` — run a 10-second pprof bundle against the
    active node; banner reports the output path on success.
  - `:logs` / `:alerts` — toggle the bottom log pane / alerts
    popup (also `Ctrl+L` / `Ctrl+A`).
  - `:help` — open the help overlay; also `?`.
  - `:quit` / `:q` — exit beegui.
- **Help overlay.** `?` (or `:help`) opens a scrollable window
  listing every keybinding and verb with usage strings.
- **Result banner.** Verbs that return a one-shot answer
  (`:hash`, `:cid`, `:diagnose`) surface their output as a
  transient banner at the bottom of the screen for 8 seconds.

### Notes

- Operational parity with bee-tui modulo bee-tui's `:upload`,
  `:batch buy/topup/dilute/extend`, `:set <knob>`, and the
  per-screen drill commands. Most of those are reachable via
  `beegui --once`; in-GUI execution is on the v0.6 backlog.

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
