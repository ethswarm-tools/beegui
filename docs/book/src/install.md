# Install

## Prebuilt installers

```sh
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/ethswarm-tools/beegui/releases/latest/download/beegui-installer.sh \
  | sh

# Windows (PowerShell)
powershell -c "irm https://github.com/ethswarm-tools/beegui/releases/latest/download/beegui-installer.ps1 | iex"
```

cargo-dist produces prebuilt binaries for:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

The installer drops the `beegui` binary into the platform's
standard local-bin location and prepends it to `PATH` for new
shells.

## From source

Needs Rust ≥ 1.85.

```sh
cargo install beegui
```

Or clone + build:

```sh
git clone https://github.com/ethswarm-tools/beegui
cd beegui
cargo run --release
```

## Linux dependencies

beegui uses egui's default eframe backend (glow / GLFW). On a fresh
Debian/Ubuntu install you may need:

```sh
sudo apt install libgl1 libglib2.0-0 libxkbcommon0 libwayland-cursor0
```

Desktop notifications use `notify-rust`'s zbus backend, which
relies on the operator's session bus — no extra packages needed
on GNOME/KDE/Hyprland.

## Verifying

```sh
beegui --version    # should print e.g. "beegui 0.12.0"
beegui --once readiness    # exits 0 if a Bee is reachable on localhost:1633
```
