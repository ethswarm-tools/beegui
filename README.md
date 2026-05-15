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

🚧 **0.1.0 unreleased — scaffold only.** The dependency on
`bee-cockpit-core` is wired up but commented out until the core's
0.1.0 ships to crates.io; see the
[extraction plan](https://github.com/ethswarm-tools/bee-cockpit-core/blob/main/PLAN.md)
for the phased rollout. First real screen (S1 Health) lands once
the core is published.

## Building

```sh
git clone https://github.com/ethswarm-tools/beegui
cd beegui
cargo run --release
```

Today's binary shows a placeholder window — the cockpit lands as
the core extraction progresses.

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE)
or [MIT license](./LICENSE-MIT) at your option.
