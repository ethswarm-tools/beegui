# beegui screenshots + screencast

This directory holds the visual assets the README + blog posts
reference. Unlike bee-tui's [VHS tapes](https://github.com/charmbracelet/vhs),
beegui is a real desktop application and there is no headless
renderer that produces faithful output — frames must be captured
from a running session on a real display server.

## What to capture

The README placeholders (and the recipe below) assume the
following set. Each one should be captured against a node that
has interesting data — not a fresh empty node, where every
screen is "waiting…".

| File | What it shows | Suggested screen |
|---|---|---|
| `cold-start.png` | Headline shot: first frame after launch, S1 Health with all gates rendered and the status bar populated. | S1 Health |
| `s2-stamps.png` | Stamps table with at least 3 batches of varied status; one row selected showing the bucket-histogram drill below. | S2 Stamps |
| `s4-lottery.png` | Round phase ribbon + anchors + stake card. Optionally with the `rchash` bench result row visible (green/red). | S4 Lottery |
| `s6-peers.png` | Bin strip + peer table with a peer selected → drill panel populated (balance, ping, settl. in/out, last cheques). | S6 Peers |
| `s10-pins.png` | Pin list after `c` (check all) has run — mix of good/bad/unchecked rows. | S10 Pins |
| `s15-fleet.png` | Multi-node roll-up with at least 2 nodes, one active (highlighted), one secondary. Bonus: caught mid-switch with the *Switch to <name>* button visible. | S15 Fleet |
| `palette.png` | Command palette open (`:` pressed) with the verb list filtered, e.g. typing `:up` shrinks the list to `:upload`. | Any screen |
| `screencast.gif` (or `.webm`) | Optional ~30 s tour: cold-start → Tab cycle → S2 drill → palette → `:hash` banner → fleet switch. | — |

For the screencast, ~30 seconds keeps the file size in the
1–3 MiB range. Aim for 24 fps and 1200 px wide (downscaled if
the source resolution is higher); egui defaults look fine at
that scale.

## Recommended tools

### Linux (Wayland — GNOME / KDE / Hyprland / Sway)

```sh
# install once
sudo apt install grim slurp ffmpeg wf-recorder      # Wayland (wlroots)
sudo apt install gnome-screenshot ffmpeg            # GNOME alt
sudo apt install spectacle ffmpeg                   # KDE alt
```

```sh
# stills — region select
grim -g "$(slurp)" docs/screenshots/cold-start.png

# stills — single window (grim alone, or use gnome-screenshot -w)
grim docs/screenshots/cold-start.png

# 30-second screencast (wf-recorder is the wlroots default)
wf-recorder -g "$(slurp)" -f docs/screenshots/screencast.mp4 \
    --duration 30
# convert to gif for README embedding (palette extraction for size)
ffmpeg -i docs/screenshots/screencast.mp4 \
    -vf "fps=15,scale=1200:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse" \
    docs/screenshots/screencast.gif
```

### Linux (X11)

```sh
sudo apt install scrot ffmpeg
scrot -s docs/screenshots/cold-start.png            # region select

# 30-second screencast with ffmpeg (replace :0.0 with your DISPLAY)
ffmpeg -video_size 1200x800 -framerate 24 -f x11grab -i :0.0+100,100 \
    -t 30 docs/screenshots/screencast.mp4
```

### macOS

```sh
# stills — built-in
# Cmd+Shift+4 for region select; saves to Desktop
# Cmd+Shift+5 opens the full capture panel (window + region + screencast)

# screencast via ffmpeg + AVFoundation (replace "1:0" with your screen index)
ffmpeg -f avfoundation -framerate 24 -i "1:0" -t 30 \
    docs/screenshots/screencast.mp4
```

### Windows

```powershell
# stills — Win+Shift+S (Snipping Tool) or Greenshot
# screencast — built-in Xbox Game Bar (Win+G) or ShareX
```

## Framing tips

- Resize the beegui window to ~1200 × 800 before capturing —
  the README renders images at that approximate width and
  larger captures get downscaled by the browser anyway.
- Use a dark theme (`beegui --theme dark`) for screencasts and
  light theme for stills, or pick whichever matches the README
  blog post style. Just stay consistent within a release.
- For drill / palette shots, trigger the drill or palette
  **before** starting the capture so the first frame already
  shows the populated state. Otherwise the GIF wastes its first
  second on "user presses a key".
- Crop / annotate sparingly. Plain captures age better than
  marked-up ones — the next release re-renders cleanly.

## Conventions

- File names are kebab-case + match the screen number where
  applicable (`s2-stamps.png`, `s15-fleet.png`).
- Commit PNGs at original capture resolution; the README
  rendering downscales on the fly. Keep individual stills
  ≤ 400 KiB and the screencast GIF ≤ 3 MiB.
- Re-capture on the *first* release that changes a screen's
  visual layout, not every release. The "cold-start" headline
  shot should track the current major; the per-screen shots
  can lag by a minor.
