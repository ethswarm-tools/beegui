# Changelog

All notable changes to **beegui** are tracked here. The
format follows [Keep a Changelog]; the project adheres to
[Semantic Versioning].

[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html

## [Unreleased]

TBD.

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
