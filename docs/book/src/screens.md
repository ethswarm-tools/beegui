# Screens

beegui has 15 screens accessible via the tab strip at the top, the
number keys `1`–`9`, or `Tab` / `Shift+Tab` to cycle. The active
screen is highlighted in the strip.

Common interactions:

- `↑ / ↓` (or `j / k`) move selection in the active list.
- `PgUp / PgDn` page selection ±10 rows; `Home / End` jump to the
  first / last row.
- `Enter` (or click) drills into the selected row.
- `Esc` closes any drill or overlay.

## S1 — Health gates

Status ladder built by `views::health::gates_for_with_stamps`:
API reachable → chain RPC fresh → wallet funded → warmup complete
→ peer count → reserve fill → bin saturation → redistribution
healthy → not frozen → stamp TTL. Each gate shows Pass / Warn /
Fail with a one-line *why*. Stamp TTL surfaces the worst-batch
TTL (the one most likely to expire).

The gate ladder also drives the alerts pipeline — every
transition that crosses a Warn / Fail boundary lands in the
alerts ring (Ctrl+A).

## S2 — Stamps

Postage batches in a sortable table: batch ID prefix, depth,
amount, TTL, status (Healthy / Skewed / Empty / Expiring).
Selecting a row (Enter or click) loads the bucket histogram drill
— per-bucket fill across the six bins, plus the top-10 worst
buckets by collision count.

The drill calls `GET /stamps/{id}/buckets` once and caches the
result; press Esc to close it.

## S3 — Swap

Chequebook balance + on-chain debit/credit + the cheques and
settlements tables. Cheques are split by direction (in / out)
with last-cashed timestamps.

## S4 — Lottery

Redistribution round state: phase ribbon (Commit / Reveal / Claim
/ Sample), block-of-round counter, anchor history, stake card.
Stake status surfaces *why*: Healthy / Skewed / Frozen / Unstaked
/ Insufficient gas.

`r` runs an [`rchash` benchmark](https://docs.ethswarm.org/) at
the health-derived storage depth against the full anchor range.
Verdict turns green under 95 s (the reveal-phase deadline) and
red above.

## S5 — Warmup

Boot sequence checklist for a freshly-started node: peers
connected → bin coverage → reserve fill → ready. Progress bars
plus an elapsed counter; useful when triaging "is Bee actually
making progress?" complaints.

## S6 — Peers

Bin saturation strip (one cell per kademlia bin, color by
fill ratio) + the peer table. Selecting a row triggers a
6-endpoint fan-out: `peer_balance`, `peer_cheques`,
`peer_settlement`, `ping_peer`, `status_peers`, local `status`.
The drill renders balance, ping, settl. in/out, last cheques
in/out, storage radius, reserve size, pullsync rate, batch
commitment (with >5% outlier flag).

## S7 — Network

Identity (overlay + underlay), NAT reachability (via AutoNAT),
listed underlays. Useful for confirming "is my node reachable
from outside?".

## S8 — API Health

Chain state (block height + sync lag), pending tx count, plus
HTTP call-stats from the cockpit's own outbound requests (count,
P50/P95/P99 latency by endpoint). The call-stats panel reads
from the same `LogCapture` that drives the bee::http tab in the
bottom log pane.

## S9 — Tags

Upload tag table: ID, progress (sent / received / synced),
counters. Useful when checking why a `bee-rs` upload is stuck.

## S10 — Pins

Pinned references with check states. Enter on a row (or `c` for
"check all") runs `GET /pins/check` per reference. `s` cycles
sort modes: by reference / bad first / total chunks.

## S11 — Manifest

Paste a root reference + click *Load*. The Mantaray tree
renders with lazy fork expansion — `▶` to expand a fork; only
the visible subtree is fetched. Useful for inspecting upload
contents without downloading the whole collection.

## S12 — Watchlist

Add references one at a time and run durability checks. Each
ref runs through `bee_cockpit_core::durability::check`; results
roll into a 50-entry history. The *Re-check all* button refreshes
every entry.

## S13 — Feed Timeline

Owner + topic + max-entries. *Walk feed* spawns
`feed_timeline::walk` and renders newest-first results with the
reference column shortened. Clicking a row reveals the full
reference hex.

## S14 — Pubsub

Live tail of PSS topic + GSOC subscriptions. The mode toggle
switches between PSS (topic-only) and GSOC (owner +
identifier). Messages land in a 200-entry ring with an optional
case-insensitive substring filter. When `[pubsub].history_file`
is set, every message also appends to a JSONL file with
size-rotation.

## S15 — Fleet

Multi-node health roll-up: one row per `[[nodes]]` entry (or
positional CLI URL), polled every 15 seconds in parallel.
Aggregate status, peer count, worst stamp TTL, `/health` ping.

Enter on a row (or double-click, or the *Switch to* button)
switches the active node — beegui tears down the BeeWatch hub
and rebuilds it against the new endpoint. The fleet poller keeps
running so the roll-up doesn't blink.

`r` re-polls the fleet immediately (kicks the resync channel).

## Bottom log pane

`Ctrl+L` toggles. Seven tabs:

- **Errors / Warning / Info / Debug** — Bee process log lines
  by severity (from the bee-log tailer; see [Bee log tailing](./bee-log.md)).
- **Bee HTTP** — Bee's own incoming `/api` request log.
- **bee::http** — the cockpit's outbound calls (the "trust
  anchor" tab).
- **Cockpit** — beegui's own tracing events.

Tab counts surface in the strip; the pane header shows the
resolved bee-log source + origin (`CLI` / `config` / `discovered`
/ `supervised`).
