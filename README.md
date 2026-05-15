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

**0.2.0** — first feature release. All 15 screens (S1–S15) are
reachable from the tab bar; 10 of them stream live data via
`BeeWatch`. The remaining 5 (Manifest, Watchlist, Feed Timeline,
Pubsub, Fleet) ship placeholder UIs and gain their workers in
v0.3.

| Screen | State |
|---|---|
| S1 Health | live — gates + Stamp TTL |
| S2 Stamps | live — table + status |
| S3 Swap | live — chequebook + cheques + settlements |
| S4 Lottery | live — round + anchors + stake |
| S5 Warmup | live — checklist + elapsed + progress bars |
| S6 Peers | live — bin strip + peer table |
| S7 Network | live — identity + reachability + underlays |
| S8 API Health | live — chain + pending tx (call-stats pending log pane) |
| S9 Tags | live — progress bars + counts |
| S10 Pins | live — list (checks pending durability worker) |
| S11 Manifest | placeholder — input + walker in v0.3 |
| S12 Watchlist | placeholder — durability worker in v0.3 |
| S13 Feed Timeline | placeholder — walker in v0.3 |
| S14 Pubsub | placeholder — subscriber in v0.3 |
| S15 Fleet | placeholder — multi-node poller in v0.3 |

## Install

```sh
cargo install beegui
```

## Usage

```sh
beegui                            # connect to http://localhost:1633
beegui --node http://host:1633    # explicit node URL
beegui --token <bearer>           # restricted-mode auth
beegui --config ~/beegui.toml     # explicit config file
beegui http://a:1633 http://b:1633  # positional URLs (ad-hoc fleet)
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
```

### Keys

| Key | Action |
|---|---|
| `1`–`9` | Jump to that screen |
| `Tab` / `Shift+Tab` | Cycle screens |
| Click a tab | Same as above |

## Building from source

```sh
git clone https://github.com/ethswarm-tools/beegui
cd beegui
cargo run --release
```

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE)
or [MIT license](./LICENSE-MIT) at your option.
