# Command palette

Open with `:` or `Ctrl+P`. Type to filter the verb list; `↑/↓`
navigates; `Enter` submits; `Esc` dismisses.

Drag-and-drop a file onto the beegui window to pre-fill
`:upload <path>` — the operator just hits Enter to ship.

## Verbs

### Navigation

| Verb | What it does |
|---|---|
| `:go <screen>` (also `:health`, `:stamps`, `:fleet`, …) | Switch to a screen by name. |
| `:nodes` (also `:node`) | Open the centered node picker. Same as `Ctrl+N`. |
| `:context <name>` (aliases `:ctx`, `:switch`) | Switch active node by typed name. |
| `:logs` | Toggle the bottom log pane. |
| `:alerts` | Toggle the alerts panel. |
| `:help` (also `:?`) | Open the help overlay. |
| `:quit` (also `:q`, `:exit`) | Exit beegui. |

### Inspection (no Bee write)

| Verb | What it does |
|---|---|
| `:hash <path>` | Compute the swarm hash of a local file. Result on the banner. |
| `:cid <ref> [manifest\|feed]` | Compute the IPFS-style CID for a swarm reference. |
| `:inspect <ref>` (alias `:manifest`) | Switch to S11 and load that root reference. |
| `:feed-timeline <owner> <topic> [max]` (alias `:ft`) | Switch to S13 and walk the feed. |
| `:durability <ref>` | Switch to S12 and add the reference + run the check. |
| `:feed-probe <owner> <topic>` (alias `:fp`) | Fetch the latest feed update; banner reports index + payload size. |
| `:diagnose` | Write a 10-second pprof bundle to `/tmp/beegui-diagnose-<ts>/`. Banner reports the path. |

### Active (Bee writes)

| Verb | What it does |
|---|---|
| `:upload <path> [batch-prefix]` | Upload a file or directory. Batch is auto-picked when no prefix is supplied. |
| `:pss <topic> <payload> [batch-prefix]` | Send a PSS message. Hex 32-byte topics pass through verbatim; anything else is keccak256-of-string. |
| `:batch buy <depth> [amount]` | Stamp-batch buy *preview*. |
| `:batch topup\|dilute\|extend <id> <arg>` | Stamp-batch topup / dilute / extend *previews*. No transactions are sent. |

## Banner

One-shot verbs (`:hash`, `:cid`, `:feed-probe`, `:diagnose`,
`:batch *`) surface their result as a transient banner at the
bottom of the screen (8 second TTL). Error states are red, OK
states green.

## Tab completion

Not yet implemented — typing a prefix narrows the suggestion
list, and `↑/↓` navigate the matches. Pressing `Enter` on an
empty input commits the currently-highlighted suggestion.
