# `--once` CLI

beegui ships the full bee-tui `--once` verb surface for CI / cron
/ scripting use. No GUI is opened; the result is printed to
stdout (text by default, JSON with `--json`) and beegui exits
with a status code:

| Code | Meaning |
|---|---|
| 0 | Ok. |
| 1 | Unhealthy / verb produced a sad result. |
| 2 | Error (network, parse, …). |
| 64 | Usage error (bad args). |

## Usage

```sh
beegui --once readiness --json http://localhost:1633
beegui --once depth-table
beegui --once hash ./somefile
beegui --once durability-check <ref>
beegui --once buy-preview --json
```

The verb takes positional arguments after it; the node URL (when
needed) is taken from `--node`, `BEE_NODE_URL`, or the trailing
positional. `--config <path>` works the same as in interactive
mode.

## Verbs

### Inspection (no Bee write)

| Verb | What it does |
|---|---|
| `readiness` | Run the gate ladder; exit 0 if green, 1 if any gate fails. |
| `version-check` (alias `check-version`) | Compare beegui's expected Bee API version against the live server. |
| `config-doctor` | Validate the config file; flag missing nodes / bad tokens / unknown keys. |
| `hash <path>` | Compute the swarm hash of a local file. |
| `cid <ref> [manifest\|feed]` | Compute the CID. |
| `depth-table` | Print the stamp depth → chunks-storable table. |
| `pss-target` | Print the recommended target for the current overlay. |
| `gsoc-mine <nonce>` | Mine a GSOC identifier for the given nonce. |
| `price` | Current chain price. |
| `basefee` | Current chain basefee. |
| `inspect <ref>` | One-shot manifest walk; report children + sizes. |
| `durability-check <ref>` | Walk the chunk graph; report bad/missing. |
| `feed-probe <owner> <topic>` | Fetch latest feed update. |
| `feed-timeline <owner> <topic> [max]` | Walk feed history. |
| `grantees-list <ref>` | List grantees of an ACT-controlled reference. |

### Stamp-math previews (no transactions)

| Verb | What it does |
|---|---|
| `buy-preview <depth> [amount]` | What would `:batch buy` cost? |
| `buy-suggest` | Recommend a depth based on current reserve fill. |
| `topup-preview <id> <amount>` | TTL extension preview. |
| `dilute-preview <id> <new-depth>` | Dilute preview. |
| `extend-preview <id> <seconds>` | Time-extend preview. |
| `plan-batch` | End-to-end batch plan (depth + amount + estimated TTL). |

### Active (Bee writes)

| Verb | What it does |
|---|---|
| `upload-file <path> [batch-prefix]` | Upload a single file. |
| `upload-collection <dir> [batch-prefix]` | Upload a directory as a Mantaray collection. |

## Output

Default is one human-readable line. `--json` emits a single
object with `verb`, `status`, `message`, and `data` fields —
parseable with `jq`.

```sh
$ beegui --once readiness --json
{"verb":"readiness","status":"ok","message":"all 11 gates pass","data":null}
```

## Why a separate CLI mode

bee-tui's `--once` exists for the same reason: operators want to
script the same logic the cockpit visualises. Drop a
`beegui --once readiness` into a cron and you have a health probe
that returns the same verdict as opening the GUI and reading the
gate ladder by eye.
