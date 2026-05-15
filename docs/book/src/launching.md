# Launching beegui

## Quickstart

```sh
beegui                            # talk to http://localhost:1633
beegui --node http://host:1633    # explicit node URL
beegui --token <bearer>           # restricted-mode auth
beegui --config ~/beegui.toml     # explicit config file
beegui --theme light              # auto | light | dark
beegui http://a:1633 http://b:1633  # positional URLs (ad-hoc fleet)
```

## CLI flags

| Flag | Effect |
|---|---|
| `--node <URL>` | Override the active node URL. Falls back to `$BEE_NODE_URL`, then `http://localhost:1633`. |
| `--token <bearer>` | Restricted-mode auth. Also reads `$BEE_NODE_TOKEN`. |
| `--config <PATH>` | Explicit config file. Without it beegui searches the platform default path. |
| `--theme <auto\|light\|dark>` | Visual scheme. Auto follows the OS. |
| `--bee-log <PATH>` | Tail an external Bee log file. See [Bee log tailing](./bee-log.md). |
| `--bee-log-cmd <CMD>` | Tail a shell command's stdout instead. |
| `--bee-bin <PATH>` | Spawn Bee as a child process. See [Bee process supervision](./supervisor.md). |
| `--bee-config <PATH>` | Path to the Bee YAML config. Required with `--bee-bin`. |
| `--once <verb>` | Run a single verb and exit (no GUI). See [`--once` CLI](./once.md). |
| `--json` | When combined with `--once`, emit JSON. |
| `<urls...>` | Positional ad-hoc fleet URLs — replaces the config's node list for the session. First URL is the active node. |

## Environment

| Env var | Effect |
|---|---|
| `BEE_NODE_URL` | Default node URL if no CLI flag and no config. |
| `BEE_NODE_TOKEN` | Default token. |
| `BEEGUI_CONFIG` | Path to the config file (alternative to `--config`). |
| `BEEGUI_DATA` | Override the data directory beegui writes state into. |
| `BEEGUI_LOG_LEVEL` | tracing filter (e.g. `debug`, `beegui=trace,reqwest=warn`). |
| `RUST_LOG` | Fallback if `BEEGUI_LOG_LEVEL` isn't set. |
| `NO_COLOR=1` | Force the dark/mono visual scheme regardless of OS. |

## Ad-hoc fleet (no config)

The shortest path to a multi-node session:

```sh
beegui http://10.0.0.1:1633 http://10.0.0.2:1633 http://10.0.0.3:1633
```

The first URL is the active node. All three appear in S15 Fleet
and `Ctrl+N`'s node picker. The fleet poller cycles them every 15
seconds; per-screen pollers (health, stamps, peers, …) only run
against the active node.

## Switching the active node

| | |
|---|---|
| `Ctrl+N` | Open the centered picker; arrows / `j`-`k` to select; Enter to switch. |
| `:nodes` | Same as Ctrl+N from the command palette. |
| `:context <name>` | Switch by typed name (no picker). |
| S15 Fleet | Enter on any row, or double-click, or the *Switch to* button. |
