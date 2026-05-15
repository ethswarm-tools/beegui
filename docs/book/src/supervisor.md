# Bee process supervision

When `--bee-bin` and `--bee-config` are set (or the `[bee]` block
is present in config), beegui forks Bee as a child process for
the session and owns its lifecycle:

```sh
beegui --bee-bin ./bee --bee-config ./bee.yaml
```

At startup:

1. The binary's existence + executability are checked.
2. A rotating log file is opened in `$TMPDIR/beegui-spawned-<ts>.log`.
3. `bee start --config <yaml>` is forked. Stdout + stderr are
   captured to the rotating writer; the child gets its own
   process group so SIGTERM-pgroup later kills the whole tree.
4. beegui polls `/health` until it returns 200, up to 60 s. Bee's
   first start can include chain-state catch-up, hence the
   generous timeout.
5. The cockpit window opens.

## Status chip

The bottom status bar (right side) shows the supervisor state:

| Label | Meaning |
|---|---|
| `● bee running` | Process alive. Green. |
| `○ bee exited (0)` | Clean exit. Grey. |
| `✕ bee exited (137)` | Non-zero exit. Red (137 typically = OOM kill). |
| `✕ bee killed (sig 9)` | Killed by signal. Red. |

Polled every frame.

## Auto-routed log

The supervisor's rotating capture file is automatically wired as
the bee-log source — Errors / Warn / Info / Debug / Bee HTTP tabs
populate without any `--bee-log` flag. The pane header shows
`[supervised]` as the source origin.

When the supervisor is active, `--bee-log` / `--bee-log-cmd` /
auto-discovery are ignored for the active node — the supervisor's
file is always the freshest source.

## Log rotation

The capture file rotates by size; defaults are 64 MiB per file,
5 retained files (≈320 MiB ceiling). Override in config:

```toml
[bee.logs]
rotate_size_mb = 128
keep_files = 10
```

Rotation is transparent to the tailer.

## Clean shutdown

`on_exit` fires when the window closes. beegui:

1. Sends `SIGTERM` to Bee's process group.
2. Waits up to 5 s for clean exit (Bee's shutdown closes RocksDB
   cleanly — rushing it leaves the DB in recovery-required state
   on next start).
3. Escalates to `SIGKILL` if Bee hasn't exited.

The terminal output reports the final status (`bee exited
cleanly` / `bee killed (signal 9)`).

## What's not (yet) supported

- **Auto-restart watchdog.** bee-tui supports
  `[bee.supervisor] auto_restart = true` for crash-loop
  recovery; beegui currently always behaves as if
  `auto_restart = false`: log the exit, dim the chip, no
  respawn. Operators restart beegui to retry.
- **Interactive restart.** No palette verb to restart Bee in
  place — quit and relaunch beegui.

Both are reasonable v1.x additions.

## When supervision is the right tool

- **Dev rigs.** beegui takes care of Bee lifecycle so you only
  manage one process.
- **Testnet/Sepolia experiments.** Especially with the bee-rs
  Sepolia integration-check rig.
- **Single-shot debugging.** Start Bee, observe its logs, close
  the window — Bee is gone too.

When *not*:

- **Production nodes.** Use systemd / Docker as the supervisor
  and have beegui connect to the API + tail logs (`--bee-log`).
- **Multi-node fleets.** Supervision is per-cockpit-session; the
  fleet view is fine but the supervisor only owns the active
  node's process.
