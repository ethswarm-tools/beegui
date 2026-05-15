# Introduction

**beegui** is a desktop GUI cockpit for [Ethereum Swarm](https://www.ethswarm.org/)
Bee node operators. It is the GUI sibling of [bee-tui]: same cockpit
logic — health gates, stamp TTLs, fleet roll-up, redistribution skip
reasons, durability checks, manifest walking, feed timelines, pubsub
watches — rendered with [egui] instead of ratatui.

[bee-tui]: https://github.com/ethswarm-tools/bee-tui
[egui]: https://github.com/emilk/egui

## What it shows

15 screens. Some are status views (health gates, bin saturation,
fleet roll-up). Some are inspectors with drill panels (per-stamp
bucket histograms, per-peer balances + cheques, manifest fork
walks). Two are subscribers (pubsub PSS/GSOC, feed timelines). One
spawns and supervises a Bee process for the session (v0.11+).

| | Screen | What it surfaces |
|---|---|---|
| S1 | Health | Gate ladder + worst-batch stamp TTL |
| S2 | Stamps | Batch table + bucket-histogram drill |
| S3 | Swap | Chequebook + cheques + settlements |
| S4 | Lottery | Round phase + anchors + stake + `rchash` bench |
| S5 | Warmup | Bootstrap checklist + progress bars |
| S6 | Peers | Bin strip + peer drill (6-endpoint fan-out) |
| S7 | Network | Identity + reachability + underlays |
| S8 | API Health | Chain state + pending tx + HTTP call-stats |
| S9 | Tags | Upload progress + counts |
| S10 | Pins | Pinned-reference inspector + integrity check |
| S11 | Manifest | Lazy Mantaray fork walker |
| S12 | Watchlist | Durability worker + result history |
| S13 | Feed Timeline | Owner+topic walker |
| S14 | Pubsub | PSS/GSOC subscriber + ring buffer + filter |
| S15 | Fleet | Multi-node roll-up + switch active |

## How it differs from bee-tui

The two share the [bee-cockpit-core] crate — every gate ladder,
every bucket histogram, every fleet roll-up comes from the same
`view_for(snapshot) -> View` functions. The widget layer is the
only thing that differs: bee-tui paints with ratatui; beegui paints
with egui. Visuals come from the same logic.

[bee-cockpit-core]: https://github.com/ethswarm-tools/bee-cockpit-core

What you'd pick beegui for:

- You prefer a native desktop window to a terminal.
- You want drag-and-drop uploads (drop a file → palette pre-fills
  `:upload <path>`).
- You want OS-native desktop notifications on gate failures.

What you'd pick bee-tui for:

- You SSH into nodes and live in the terminal.
- You want every screen scrollable with vim-style keys + a longer
  command bar.
