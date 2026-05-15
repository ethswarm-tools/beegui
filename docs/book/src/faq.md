# FAQ

### How is beegui different from bee-tui?

Both speak the same logic from `bee-cockpit-core` (gates, drills,
fleet roll-up, …). bee-tui paints with ratatui in a terminal;
beegui paints with egui in a native desktop window. Pick beegui
when you'd prefer the window — clickable rows, drag-and-drop
uploads, OS notifications.

### Where is the config file?

`~/.config/beegui/config.toml` on Linux. macOS uses
`~/Library/Application Support/beegui/config.toml`; Windows uses
`%APPDATA%\beegui\config.toml`. Override with `--config <path>`
or `BEEGUI_CONFIG`.

### Why is the bottom log pane mostly empty?

The Errors / Warn / Info / Debug / Bee HTTP tabs depend on a
bee-log source. Without one (no `--bee-log`, no `[[nodes]]`
log_file/log_command, no `--bee-bin`, no Linux auto-discovery
match), those tabs stay empty. The bee::http tab (cockpit's
outbound calls) and the Cockpit tab (beegui's own tracing) are
populated regardless.

The pane header shows the resolved source — if it says
"(no bee-log source)" the empty tabs are expected.

### Why doesn't auto-discovery work for my remote Bee?

It's Linux-only and local-only: it walks the host's `/proc` to
find the Bee process behind the URL. Remote URLs and non-Linux
hosts fall through. Set `[[nodes]].log_command` to a
`ssh remote 'tail -f /var/log/bee.log'` or the equivalent for
your setup.

### Why does my screencast/screenshot directory look empty?

Because we haven't captured them yet. `docs/screenshots/` ships
with just the capture recipe. Captures are user-side — beegui is
a real desktop app and there's no faithful headless renderer.

### Can I get desktop notifications across SSH?

Not for the OS-native ones. `notify-rust` talks to the local
session bus (libnotify on Linux, Notification Center on macOS).
For remote-node notifications use the `[alerts] webhook_url`
instead, which fires Slack-compatible webhooks.

### How do I switch nodes mid-session?

`Ctrl+N` opens the picker from anywhere. `:context <name>` does
the same from the palette. S15 Fleet shows every node; Enter on
a row switches.

### Does beegui restart Bee if it crashes?

Not yet — the supervisor in v0.11 always behaves as if
`auto_restart = false`. The status chip turns red on exit; you
restart beegui to retry. bee-tui has the watchdog; beegui's
catch-up is a v1.x item.

### Where does beegui write data?

`~/.local/share/beegui/` on Linux (or `$XDG_DATA_HOME/beegui`).
macOS/Windows equivalents follow the platform conventions.
Override with `BEEGUI_DATA`.

### Why does `cargo install beegui` take a while?

eframe pulls in glow, winit, glutin, and a chain of windowing
deps. First build on a fresh machine is ~5 minutes; subsequent
incremental builds are seconds.

### Why is my Ctrl+L log-pane toggle eaten by something else?

Some terminal emulators bind Ctrl+L to clear-screen and pass
that to the foreground app. beegui is a native window, not a
terminal app, so this shouldn't happen — but check that the
keyboard focus is on beegui's window and not e.g. a transparent
terminal you forgot was on top.

### Where do I file a bug?

<https://github.com/ethswarm-tools/beegui/issues>. Mention the
beegui version (`beegui --version`), the OS, and either
reproduce steps or relevant log lines from the Cockpit tab.
