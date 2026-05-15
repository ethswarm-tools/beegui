# Bee log tailing

beegui can tail Bee's own log output and surface it in the bottom
log pane alongside the cockpit's own `bee::http` events. Bee's
log is the operator's primary source of truth for things the API
doesn't reveal — kademlia kicks, batch updates, postage stamp
errors, RocksDB compactions.

## Source priority

beegui picks one source per node. Highest priority wins:

1. **Supervisor** — when `--bee-bin` is set, the supervisor's
   rotating capture file is the source.
2. **CLI flag** — `--bee-log <path>` or `--bee-log-cmd <cmd>`.
3. **Config** — `[[nodes]].log_file` or `[[nodes]].log_command`.
4. **Auto-discovery** — Linux only, local nodes only. See below.
5. **None** — the Bee-side tabs stay empty; the pane header
   explains why.

Within a tier, a command beats a file.

## CLI flags

```sh
beegui --bee-log /var/log/bee/bee.log
beegui --bee-log-cmd "journalctl -u bee -f"
beegui --bee-log-cmd "docker logs -f bee 2>&1"
beegui --bee-log-cmd "ssh bee-host 'tail -f /var/log/bee.log'"
```

`--bee-log` tails from EOF — pre-existing history doesn't replay.
The tailer survives log rotation (`logrotate`-style inode swaps)
and truncation.

`--bee-log-cmd` runs the command through `sh -c`, so pipes /
quoting / redirects in the operator's string behave as typed.
The child is killed on quit. Stderr is discarded — sources that
write to stderr (e.g. `docker logs`) should redirect with `2>&1`.

## Per-node config

```toml
[[nodes]]
name = "production"
url = "http://bee-prod.internal:1633"
log_command = "ssh bee-prod.internal 'tail -f /var/log/bee.log'"

[[nodes]]
name = "supervised"
url = "http://localhost:1733"
log_file = "/var/log/bee/bee.log"
```

When switching nodes (Ctrl+N or S15 Enter), beegui re-resolves
the source against the new node and respawns the tailer. The
log-pane rings clear so stale lines don't bleed across nodes.

## Auto-discovery (Linux only)

When no explicit source is set, beegui walks `/proc` to find the
Bee process behind the active node URL:

1. Parse the node URL — must be a loopback host (`localhost`,
   `127.0.0.1`, `::1`).
2. Find the PID listening on the API port (from
   `/proc/net/tcp{,6}`).
3. Classify `/proc/<pid>/fd/1` (Bee logs to stdout):
   - **Regular file** → tail it directly.
   - **Pipe under docker** → `docker logs -f <id>`.
   - **Pipe under systemd** → `journalctl -u <unit> -f`.
   - **TTY / null / opaque pipe** → can't capture; the pane
     header explains and suggests `log_command`.

Non-Linux hosts and remote URLs fall through to the no-source
placeholder.

## The seven log tabs

`Ctrl+L` opens the pane. Tabs (from the parser in
`bee_cockpit_core::bee_log`):

| Tab | Source |
|---|---|
| Errors | Bee log lines with `level=error`. |
| Warning | `level=warning`. |
| Info | `level=info`. |
| Debug | `level=debug`. |
| Bee HTTP | Bee's own incoming API request log lines. |
| bee::http | The cockpit's outbound calls (a different stream). |
| Cockpit | beegui's own tracing events (`tracing::info!` and friends). |

The bee::http and Cockpit tabs are populated whether or not a
bee-log source is configured — they're powered by the cockpit's
own tracing capture, not by an external file.

## Filtering

Severity-tab routing already filters by `level`. Free-text
filtering inside a tab isn't implemented yet (it is in bee-tui's
`/` filter — beegui will catch up in a later release).
