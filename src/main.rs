//! beegui — desktop GUI cockpit for Ethereum Swarm Bee node operators.
//!
//! Sibling of [bee-tui]. The cockpit logic — health gates, stamp
//! warnings, fleet roll-up, redistribution skip reasons — lives in
//! [bee-cockpit-core]; this crate renders it with [egui] instead of
//! ratatui.
//!
//! [bee-tui]: https://github.com/ethswarm-tools/bee-tui
//! [bee-cockpit-core]: https://github.com/ethswarm-tools/bee-cockpit-core
//! [egui]: https://github.com/emilk/egui

mod alerts;
mod bee_log;
mod once;
mod palette;
mod screens;

use std::path::PathBuf;
use std::sync::Arc;

use std::time::Duration;

use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::config::{
    Config, ConfigPaths, NodeConfig, load_raw, nodes_from_urls, normalize_url,
};
use bee_cockpit_core::fleet::{FleetSnapshot, spawn_poller};
use bee_cockpit_core::config::{AlertsConfig, BeeConfig, NotificationsConfig};
use bee_cockpit_core::bee_supervisor::{BeeStatus, BeeSupervisor};
use bee_cockpit_core::log_capture::{self, LogCapture};
use bee_cockpit_core::views::health::gates_for_with_stamps;
use bee_cockpit_core::watch::BeeWatch;
use clap::Parser;
use screens::{Screen, ScreenState};
use tokio::runtime::Runtime;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

const FLEET_POLL_INTERVAL: Duration = Duration::from_secs(15);

fn init_logging() -> LogCapture {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::{EnvFilter, prelude::*};
    let env_filter = EnvFilter::try_from_env("BEEGUI_LOG_LEVEL")
        .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let capture = log_capture::install(200);
    let cockpit = log_capture::install_cockpit(500);
    let layer = log_capture::CaptureLayer::new(capture.clone(), cockpit);
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(layer)
        .with(ErrorLayer::default())
        .try_init();
    capture
}

pub(crate) const PATHS: ConfigPaths = ConfigPaths {
    app_name: "beegui",
    config_env: "BEEGUI_CONFIG",
    data_env: "BEEGUI_DATA",
};

const DEFAULT_URL: &str = "http://localhost:1633";

/// Desktop GUI cockpit for Ethereum Swarm Bee node operators.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Path to a config file. Falls back to the cross-platform
    /// search path when omitted.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Bee node URL. Overrides the active node from config. Falls
    /// back to $BEE_NODE_URL, then http://localhost:1633.
    #[arg(long)]
    node: Option<String>,
    /// Optional bearer token. Overrides any token from config.
    /// Also reads $BEE_NODE_TOKEN.
    #[arg(long)]
    token: Option<String>,
    /// Visual theme: `auto` (follows OS), `light`, or `dark`.
    /// Overrides `[ui].theme` from config; default is `auto`.
    #[arg(long)]
    theme: Option<String>,
    /// Run a single verb and exit (no GUI). See `--once help`
    /// for the verb list (parity with bee-tui).
    #[arg(long)]
    once: Option<String>,
    /// Emit `--once` output as a JSON object instead of a
    /// human-readable line.
    #[arg(long)]
    json: bool,
    /// Ad-hoc node URLs (positional). When supplied, beegui ignores
    /// the config's node list and uses these instead. Mirrors
    /// bee-tui's positional-URL fleet flow.
    urls: Vec<String>,
    /// Path to the Bee process's log file. Tail it into the
    /// Errors / Warn / Info / Debug / Bee HTTP tabs of the
    /// bottom log pane. Overrides `[bee].log_file` from config.
    #[arg(long = "bee-log", value_name = "PATH")]
    bee_log: Option<PathBuf>,
    /// Shell command whose stdout streams Bee's logs
    /// (e.g. `journalctl -u bee -f`). Overrides
    /// `[bee].log_command` from config.
    #[arg(long = "bee-log-cmd", value_name = "CMD")]
    bee_log_cmd: Option<String>,
    /// Path to the `bee` binary. When set together with
    /// `--bee-config`, beegui spawns Bee as a child process and
    /// waits for it to come up before opening the cockpit. Bee's
    /// stdout+stderr land in the bottom log pane automatically;
    /// SIGTERM is sent at quit. Overrides `[bee].bin` from config.
    #[arg(long = "bee-bin", value_name = "PATH")]
    bee_bin: Option<PathBuf>,
    /// Path to the Bee YAML config file to start with.
    /// Required when `--bee-bin` is set unless `[bee].config` is
    /// in the config file. Overrides `[bee].config`.
    #[arg(long = "bee-config", value_name = "PATH")]
    bee_config: Option<PathBuf>,
}

fn main() -> Result<std::process::ExitCode, color_eyre::Report> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let log_capture = init_logging();

    if let Some(verb) = cli.once.clone() {
        let rt = Runtime::new()?;
        let args = cli.urls.clone();
        let code = rt.block_on(once::run(&verb, &args, cli.json, cli.config.clone()));
        return Ok(code);
    }

    let resolved = resolve_nodes(&cli)?;
    let alerts_cfg = resolved.alerts.clone();
    let notif_cfg = resolved.notifications.clone();
    let ui_theme = resolved.theme;

    let runtime = Runtime::new()?;
    let cancel = CancellationToken::new();
    let active = resolved.nodes.active.clone();
    let url = active.url.clone();

    // Optional Bee process supervision. CLI flags win; otherwise
    // pull from [bee] in the config. Both bin+config are required;
    // partial config is a hard error so a typo doesn't silently
    // skip the spawn.
    let bee_bin_path = cli.bee_bin.clone().or_else(|| resolved.bee.as_ref().map(|b| b.bin.clone()));
    let bee_config_path = cli.bee_config.clone().or_else(|| resolved.bee.as_ref().map(|b| b.config.clone()));
    let bee_logs_cfg = resolved.bee.as_ref().map(|b| b.logs.clone()).unwrap_or_default();
    let mut supervisor: Option<BeeSupervisor> = match (bee_bin_path, bee_config_path) {
        (Some(bin), Some(cfg)) => {
            eprintln!("beegui: spawning bee {bin:?} --config {cfg:?}");
            let mut sup = BeeSupervisor::spawn(&bin, &cfg, bee_logs_cfg)?;
            eprintln!(
                "beegui: log → {} (will appear in the bottom log pane)",
                sup.log_path().display()
            );
            eprintln!(
                "beegui: waiting for {url} to respond on /health (up to 60s)..."
            );
            runtime.block_on(sup.wait_for_api(&url, Duration::from_secs(60)))?;
            eprintln!("beegui: bee ready, opening cockpit");
            Some(sup)
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(color_eyre::eyre::eyre!(
                "--bee-bin and --bee-config (or [bee].bin and [bee].config) must both be set"
            ));
        }
        (None, None) => None,
    };

    let api = Arc::new(ApiClient::from_node(&active)?);
    let watch_cancel = cancel.child_token();
    let watch = {
        let _guard = runtime.enter();
        BeeWatch::start(api.clone(), &watch_cancel)
    };
    let rt_handle = runtime.handle().clone();
    let rt_handle_clone = rt_handle.clone();
    let (fleet_rx, fleet_resync) = if resolved.nodes.all.len() > 1 {
        let _guard = runtime.enter();
        let (rx, resync) = spawn_poller(
            resolved.nodes.all.clone(),
            cancel.child_token(),
            FLEET_POLL_INTERVAL,
        );
        (Some(rx), Some(resync))
    } else {
        (None, None)
    };

    let bee_log_cli_file = cli.bee_log.clone();
    let bee_log_cli_cmd = cli.bee_log_cmd.clone();
    if supervisor.is_some()
        && (bee_log_cli_file.is_some() || bee_log_cli_cmd.is_some())
    {
        eprintln!(
            "beegui: --bee-log/--bee-log-cmd ignored because --bee-bin is set; \
             the supervisor's log file is the freshest source"
        );
    }
    let mut bee_logs = bee_log::BeeLogs::new();
    {
        let _guard = runtime.enter();
        // When the supervisor is active, its log file is the
        // authoritative source — overrides CLI/config/discovery.
        let source = if let Some(sup) = &supervisor {
            bee_log::ResolvedSource::File {
                path: sup.log_path().to_path_buf(),
                origin: bee_log::SourceOrigin::Supervisor,
            }
        } else {
            bee_log::resolve_source(
                bee_log_cli_file.as_deref().and_then(|p| p.to_str()),
                bee_log_cli_cmd.as_deref(),
                &active,
            )
        };
        bee_logs.respawn(source, watch_cancel.clone());
    }
    // Once the App owns the supervisor handle we transfer it; on quit
    // the App's on_exit hook shuts it down.
    let supervisor_handle = supervisor.take();

    let app = App {
        url,
        active_name: active.name,
        api,
        rt_handle,
        nodes: resolved.nodes.all,
        cancel: cancel.clone(),
        watch_cancel,
        watch,
        fleet_rx,
        fleet_resync,
        log_capture,
        pending_quit: false,
        log_pane_open: false,
        log_pane_tab: bee_cockpit_core::bee_log::LogTab::SelfHttp,
        bee_logs,
        bee_log_cli_file,
        bee_log_cli_cmd,
        supervisor: supervisor_handle,
        bee_status: BeeStatus::Running,
        alerts_cfg: alerts_cfg.clone(),
        notif_cfg: notif_cfg.clone(),
        alerts: alerts::AlertsPipeline::new(alerts_cfg, notif_cfg, Some(rt_handle_clone)),
        alerts_open: false,
        palette: palette::Palette::default(),
        help_open: false,
        node_picker_open: false,
        node_picker_sel: 0,
        screen: Screen::Health,
        state: ScreenState::default(),
        _runtime: runtime,
        _cancel: cancel,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        "beegui",
        options,
        Box::new(move |cc| {
            ui_theme.apply(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| color_eyre::eyre::eyre!("eframe: {e}"))?;
    Ok(std::process::ExitCode::SUCCESS)
}

struct ResolvedNodes {
    active: NodeConfig,
    all: Vec<NodeConfig>,
}

#[derive(Debug, Clone, Copy)]
enum Theme {
    Auto,
    Light,
    Dark,
}

impl Theme {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "auto" | "default" => Some(Theme::Auto),
            "light" => Some(Theme::Light),
            "dark" | "mono" => Some(Theme::Dark),
            _ => None,
        }
    }
    fn apply(self, ctx: &egui::Context) {
        match self {
            Theme::Auto => ctx.set_visuals(default_visuals(ctx)),
            Theme::Light => ctx.set_visuals(egui::Visuals::light()),
            Theme::Dark => ctx.set_visuals(egui::Visuals::dark()),
        }
    }
}

fn no_color_env() -> bool {
    // Per no-color.org: presence is what matters; any non-empty
    // value (including "0", "false") should force the colorless
    // path. Operators set NO_COLOR=1 by convention.
    std::env::var_os("NO_COLOR").map(|v| !v.is_empty()).unwrap_or(false)
}

fn default_visuals(ctx: &egui::Context) -> egui::Visuals {
    match ctx.style().visuals.dark_mode {
        true => egui::Visuals::dark(),
        false => egui::Visuals::light(),
    }
}

struct Resolved {
    nodes: ResolvedNodes,
    alerts: AlertsConfig,
    notifications: NotificationsConfig,
    theme: Theme,
    bee: Option<BeeConfig>,
}

fn resolve_nodes(cli: &Cli) -> color_eyre::Result<Resolved> {
    let token_override = cli
        .token
        .clone()
        .or_else(|| std::env::var("BEE_NODE_TOKEN").ok());

    let cfg = match load_raw::<Config>(&PATHS, cli.config.as_deref()) {
        Ok(c) => Some(c),
        Err(e) => {
            if cli.config.is_some() {
                return Err(color_eyre::eyre::eyre!("config: {e}"));
            }
            None
        }
    };
    let alerts_cfg = cfg
        .as_ref()
        .map(|c| c.alerts.clone())
        .unwrap_or_default();
    let notif_cfg = cfg
        .as_ref()
        .map(|c| c.notifications.clone())
        .unwrap_or_default();
    // NO_COLOR=1 wins over the CLI flag and config (the env is the
    // operator's "I have a strict policy about this" signal — same
    // semantics as bee-tui and the no-color.org spec).
    let theme = if no_color_env() {
        Theme::Dark
    } else {
        cli.theme
            .as_deref()
            .and_then(Theme::parse)
            .or_else(|| cfg.as_ref().and_then(|c| Theme::parse(&c.ui.theme)))
            .unwrap_or(Theme::Auto)
    };
    let bee = cfg.as_ref().and_then(|c| c.bee.clone());

    if let Some(url) = &cli.node {
        let node = NodeConfig {
            name: "cli".into(),
            url: normalize_url(url),
            token: token_override,
            log_file: None,
            log_command: None,
            default: true,
        };
        return Ok(Resolved {
            nodes: ResolvedNodes {
                active: node.clone(),
                all: vec![node],
            },
            alerts: alerts_cfg,
            notifications: notif_cfg,
            theme,
            bee,
        });
    }
    if !cli.urls.is_empty() {
        let mut all = nodes_from_urls(&cli.urls);
        if let Some(t) = token_override.clone() {
            for n in &mut all {
                if n.token.is_none() {
                    n.token = Some(t.clone());
                }
            }
        }
        let active = all
            .iter()
            .find(|n| n.default)
            .cloned()
            .unwrap_or_else(|| all[0].clone());
        return Ok(Resolved {
            nodes: ResolvedNodes { active, all },
            alerts: alerts_cfg,
            notifications: notif_cfg,
            theme,
            bee,
        });
    }
    if let Ok(url) = std::env::var("BEE_NODE_URL") {
        let node = NodeConfig {
            name: "env".into(),
            url: normalize_url(&url),
            token: token_override,
            log_file: None,
            log_command: None,
            default: true,
        };
        return Ok(Resolved {
            nodes: ResolvedNodes {
                active: node.clone(),
                all: vec![node],
            },
            alerts: alerts_cfg,
            notifications: notif_cfg,
            theme,
            bee,
        });
    }

    if let Some(c) = cfg.as_ref()
        && !c.nodes.is_empty()
    {
        let mut all = c.nodes.clone();
        if let Some(t) = token_override.clone() {
            for n in &mut all {
                if n.token.is_none() {
                    n.token = Some(t.clone());
                }
            }
        }
        let active = c.active_node().cloned().unwrap_or_else(|| all[0].clone());
        return Ok(Resolved {
            nodes: ResolvedNodes { active, all },
            alerts: alerts_cfg,
            notifications: notif_cfg,
            theme,
            bee,
        });
    }

    let node = NodeConfig {
        name: "default".into(),
        url: DEFAULT_URL.to_string(),
        token: token_override,
        log_file: None,
        log_command: None,
        default: true,
    };
    Ok(Resolved {
        nodes: ResolvedNodes {
            active: node.clone(),
            all: vec![node],
        },
        alerts: alerts_cfg,
        notifications: notif_cfg,
        theme,
        bee,
    })
}

struct App {
    url: String,
    active_name: String,
    api: Arc<ApiClient>,
    rt_handle: tokio::runtime::Handle,
    nodes: Vec<NodeConfig>,
    cancel: CancellationToken,
    watch_cancel: CancellationToken,
    watch: BeeWatch,
    fleet_rx: Option<watch::Receiver<FleetSnapshot>>,
    fleet_resync: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    log_capture: LogCapture,
    pending_quit: bool,
    log_pane_open: bool,
    log_pane_tab: bee_cockpit_core::bee_log::LogTab,
    bee_logs: bee_log::BeeLogs,
    bee_log_cli_file: Option<PathBuf>,
    bee_log_cli_cmd: Option<String>,
    supervisor: Option<BeeSupervisor>,
    bee_status: BeeStatus,
    alerts_cfg: AlertsConfig,
    notif_cfg: NotificationsConfig,
    alerts: alerts::AlertsPipeline,
    alerts_open: bool,
    palette: palette::Palette,
    help_open: bool,
    node_picker_open: bool,
    node_picker_sel: usize,
    screen: Screen,
    state: ScreenState,
    _runtime: Runtime,
    _cancel: CancellationToken,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
        self.handle_keys(ctx);
        self.observe_alerts();
        self.handle_dropped_files(ctx);
        self.bee_logs.drain();
        if let Some(sup) = self.supervisor.as_mut() {
            self.bee_status = sup.status();
        }
        let pumped = self.palette.pump();
        for a in pumped {
            self.apply_palette_action(a, frame);
        }

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (i, screen) in Screen::all().into_iter().enumerate() {
                    let label = if i < 9 {
                        format!("{}  {}", i + 1, screen.label())
                    } else {
                        screen.label().to_string()
                    };
                    let selected = self.screen == screen;
                    if ui.selectable_label(selected, label).clicked() {
                        self.screen = screen;
                    }
                }
            });
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("●").color(self.connection_dot()));
                ui.label(egui::RichText::new(&self.url).monospace().weak());
                let arrow = if self.log_pane_open { "▼" } else { "▲" };
                if ui
                    .button(format!("{arrow} Logs"))
                    .on_hover_text("Toggle the bottom log pane (Ctrl+L)")
                    .clicked()
                {
                    self.log_pane_open = !self.log_pane_open;
                }
                let unread = self.alerts.unread_count();
                let total = self.alerts.len();
                let alerts_label = if unread > 0 {
                    format!("🔔 Alerts ({unread} new)")
                } else if total > 0 {
                    format!("🔔 Alerts ({total})")
                } else {
                    "🔔 Alerts".into()
                };
                let alerts_text = if unread > 0 {
                    egui::RichText::new(alerts_label)
                        .color(egui::Color32::from_rgb(0xe0, 0xb0, 0x30))
                        .strong()
                } else {
                    egui::RichText::new(alerts_label)
                };
                if ui
                    .button(alerts_text)
                    .on_hover_text("Toggle alerts panel (Ctrl+A)")
                    .clicked()
                {
                    self.alerts_open = !self.alerts_open;
                    if self.alerts_open {
                        self.alerts.mark_read();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("beegui {}", env!("CARGO_PKG_VERSION")))
                            .weak()
                            .small(),
                    );
                    if self.supervisor.is_some() {
                        let (label, color) = supervisor_chip(&self.bee_status);
                        ui.label(
                            egui::RichText::new(label)
                                .color(color)
                                .monospace()
                                .small(),
                        );
                    }
                });
            });
        });

        if self.log_pane_open {
            egui::TopBottomPanel::bottom("logs")
                .resizable(true)
                .default_height(220.0)
                .show(ctx, |ui| {
                    draw_log_pane(
                        ui,
                        &self.log_capture,
                        &self.bee_logs,
                        &mut self.log_pane_tab,
                    );
                });
        }

        if self.alerts_open {
            let mut clear = false;
            egui::Window::new("Alerts")
                .open(&mut self.alerts_open)
                .default_width(560.0)
                .default_height(360.0)
                .show(ctx, |ui| {
                    let webhook = self.alerts.webhook_configured();
                    let desktop = self.alerts.desktop_enabled();
                    ui.horizontal(|ui| {
                        let wh = if webhook { "✔ webhook" } else { "○ no webhook" };
                        let ds = if desktop { "✔ desktop" } else { "○ no desktop" };
                        ui.label(
                            egui::RichText::new(wh)
                                .small()
                                .color(if webhook {
                                    egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)
                                } else {
                                    egui::Color32::GRAY
                                }),
                        );
                        ui.label(
                            egui::RichText::new(ds)
                                .small()
                                .color(if desktop {
                                    egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)
                                } else {
                                    egui::Color32::GRAY
                                }),
                        );
                    });
                    if self.alerts.len() == 0 {
                        ui.label(
                            egui::RichText::new("no alerts captured yet.")
                                .italics()
                                .weak(),
                        );
                        return;
                    }
                    if ui.button("Clear").clicked() {
                        clear = true;
                    }
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for ta in self.alerts.history() {
                                draw_alert(ui, ta);
                            }
                        });
                });
            if clear {
                self.alerts.clear();
            }
        }

        let mut screen_outcome = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            let out = screens::draw(
                self.screen,
                ui,
                &self.watch,
                &mut self.state,
                screens::DrawContext {
                    url: &self.url,
                    active_name: &self.active_name,
                    api: self.api.clone(),
                    rt: self.rt_handle.clone(),
                    fleet_rx: self.fleet_rx.as_ref(),
                    fleet_resync: self.fleet_resync.as_ref(),
                    log_capture: &self.log_capture,
                    watch_cancel: &self.watch_cancel,
                },
            );
            screen_outcome = Some(out);
            if let Some(banner) = self.palette.banner().cloned() {
                let painter = ui.painter();
                let rect = ui.max_rect();
                let bar_h = 28.0;
                let banner_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.bottom() - bar_h),
                    egui::vec2(rect.width(), bar_h),
                );
                painter.rect_filled(banner_rect, 0.0, banner.level.color().gamma_multiply(0.18));
                painter.text(
                    banner_rect.left_center() + egui::vec2(12.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &banner.text,
                    egui::FontId::monospace(13.0),
                    banner.level.color(),
                );
            }
        });

        self.draw_palette(ctx);
        self.draw_help(ctx);
        self.draw_node_picker(ctx);

        if let Some(out) = screen_outcome
            && let Some(name) = out.switch_to_node
        {
            self.switch_active_node(&name);
        }
        if self.pending_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(sup) = self.supervisor.take() {
            eprintln!("beegui: SIGTERM → bee (5s grace, then SIGKILL)");
            let status = self.rt_handle.block_on(sup.shutdown_default());
            eprintln!("beegui: {}", status.label());
        }
    }
}

impl App {
    fn handle_keys(&mut self, ctx: &egui::Context) {
        let palette_open = self.palette.open;
        let help_open = self.help_open;
        // When a text input owns focus, global shortcuts (digits, Tab,
        // arrows, j/k) must NOT fire — the operator is typing into the
        // field, not driving navigation.
        let text_focused = ctx.memory(|m| m.focused().is_some());
        let mut next_screen: Option<Screen> = None;
        let mut toggle_logs = false;
        let mut toggle_alerts = false;
        let mut toggle_help = false;
        let mut open_palette = false;
        let mut close_help = false;
        let mut open_picker = false;
        ctx.input(|i| {
            if palette_open {
                return;
            }
            if i.key_pressed(egui::Key::Escape) && help_open {
                close_help = true;
            }
            // Ctrl-modified shortcuts work even when typing in a text
            // input (Ctrl+P / Ctrl+L / Ctrl+A are unambiguous).
            let typed_colon = !text_focused
                && i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == ":"));
            if typed_colon || (i.modifiers.ctrl && i.key_pressed(egui::Key::P)) {
                open_palette = true;
                return;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::L) {
                toggle_logs = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::A) {
                toggle_alerts = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::N) {
                open_picker = true;
            }
            if !text_focused
                && (i.key_pressed(egui::Key::Questionmark)
                    || (i.modifiers.shift && i.key_pressed(egui::Key::Slash)))
            {
                toggle_help = true;
            }
            // Screen navigation only when nothing's typing.
            if text_focused {
                return;
            }
            if i.key_pressed(egui::Key::Tab) {
                let screens = Screen::all();
                let idx = self.screen.index();
                let next = if i.modifiers.shift {
                    (idx + screens.len() - 1) % screens.len()
                } else {
                    (idx + 1) % screens.len()
                };
                if let Some(s) = Screen::from_index(next) {
                    next_screen = Some(s);
                }
            }
            for (n, key) in [
                (1, egui::Key::Num1),
                (2, egui::Key::Num2),
                (3, egui::Key::Num3),
                (4, egui::Key::Num4),
                (5, egui::Key::Num5),
                (6, egui::Key::Num6),
                (7, egui::Key::Num7),
                (8, egui::Key::Num8),
                (9, egui::Key::Num9),
            ] {
                if i.key_pressed(key) {
                    if let Some(s) = Screen::from_index(n - 1) {
                        next_screen = Some(s);
                    }
                }
            }
        });
        if open_palette {
            self.palette.open();
        }
        if toggle_logs {
            self.log_pane_open = !self.log_pane_open;
        }
        if toggle_alerts {
            self.alerts_open = !self.alerts_open;
            if self.alerts_open {
                self.alerts.mark_read();
            }
        }
        if toggle_help {
            self.help_open = !self.help_open;
        }
        if close_help {
            self.help_open = false;
        }
        if open_picker {
            self.node_picker_open = !self.node_picker_open;
            if self.node_picker_open {
                self.node_picker_sel = self
                    .nodes
                    .iter()
                    .position(|n| n.name == self.active_name)
                    .unwrap_or(0);
            }
        }
        if let Some(s) = next_screen {
            self.screen = s;
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        let first = dropped[0].clone();
        let extra = dropped.len() - 1;
        // Always quote — paths with spaces would otherwise be split
        // by the palette's whitespace tokenizer.
        self.palette.open();
        self.palette.input = format!(":upload {:?}", first.display().to_string());
        if extra > 0 {
            self.palette.set_banner(
                palette::BannerLevel::Err,
                format!(
                    "{extra} additional file{s} ignored — drop one at a time",
                    s = if extra == 1 { "" } else { "s" }
                ),
            );
        }
    }

    fn observe_alerts(&mut self) {
        let health = self.watch.health().borrow().clone();
        let topology = self.watch.topology().borrow().clone();
        let stamps = self.watch.stamps().borrow().clone();
        let gates = gates_for_with_stamps(&health, Some(&topology), Some(&stamps));
        self.alerts.observe(&gates);
    }

    /// Tear down the current BeeWatch and rebuild it against the
    /// supplied node profile. Called by the Fleet screen when the
    /// operator Enters a row. Mirrors bee-tui's `SwitchContext`.
    fn switch_active_node(&mut self, name: &str) {
        let Some(node) = self.nodes.iter().find(|n| n.name == name).cloned() else {
            self.palette
                .set_banner(palette::BannerLevel::Err, format!("no node named {name:?}"));
            return;
        };
        if node.name == self.active_name {
            return;
        }
        let api = match ApiClient::from_node(&node) {
            Ok(a) => Arc::new(a),
            Err(e) => {
                self.palette.set_banner(
                    palette::BannerLevel::Err,
                    format!("switch {name}: {e}"),
                );
                return;
            }
        };
        // Cancel the existing watch and spawn a fresh one against the
        // new endpoint. The old pollers wind down on their own; the
        // log-capture ring is kept (operators may want to inspect the
        // last few calls to the previous node).
        self.watch_cancel.cancel();
        let new_cancel = self.cancel.child_token();
        let watch = {
            let _guard = self.rt_handle.enter();
            BeeWatch::start(api.clone(), &new_cancel)
        };
        self.api = api;
        self.url = node.url.clone();
        self.active_name = node.name.clone();
        self.watch_cancel = new_cancel.clone();
        self.watch = watch;
        // Reset alerts state so the first frame's "Unknown → X"
        // transitions don't fire as bogus recoveries.
        self.alerts = alerts::AlertsPipeline::new(
            self.alerts_cfg.clone(),
            self.notif_cfg.clone(),
            Some(self.rt_handle.clone()),
        );
        // Re-resolve the bee-log source for the new node and
        // respawn the tailer under the new watch_cancel scope.
        let source = bee_log::resolve_source(
            self.bee_log_cli_file.as_deref().and_then(|p| p.to_str()),
            self.bee_log_cli_cmd.as_deref(),
            &node,
        );
        {
            let _guard = self.rt_handle.enter();
            self.bee_logs.respawn(source, new_cancel);
        }
        // Drop node-bound screen state: drill panels showing the old
        // node's bucket histograms, the old node's loaded Mantaray
        // tree, the old node's peer drill, etc. Watchlist state is
        // reference-keyed and node-agnostic, so we *don't* reset it.
        self.state.stamps = screens::stamps::StampsScreenState::default();
        self.state.peers = screens::peers::PeersScreenState::default();
        self.state.pins = screens::pins::PinsScreenState::default();
        self.state.manifest = screens::manifest::ManifestState::default();
        self.state.feed_timeline = screens::feed_timeline::FeedTimelineState::default();
        self.state.pubsub = screens::pubsub::PubsubState::default();
        self.state.lottery = screens::lottery::LotteryScreenState::default();
        self.state.warmup = screens::warmup::WarmupState::default();
        self.palette.set_banner(
            palette::BannerLevel::Ok,
            format!("switched to {} ({})", node.name, node.url),
        );
    }

    fn draw_palette(&mut self, ctx: &egui::Context) {
        if !self.palette.open {
            return;
        }
        // ESC closes; Up/Down navigate; Enter submits.
        let mut submit = false;
        let mut close = false;
        let mut sel_prev = false;
        let mut sel_next = false;
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                close = true;
            }
            if i.key_pressed(egui::Key::Enter) {
                submit = true;
            }
            if i.key_pressed(egui::Key::ArrowUp) {
                sel_prev = true;
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                sel_next = true;
            }
        });
        let mut actions: Vec<palette::PaletteAction> = Vec::new();
        egui::Area::new(egui::Id::new("palette"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(560.0);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.palette.input)
                            .hint_text(": type a verb…  (Esc to close, ↑/↓ to pick)")
                            .desired_width(540.0)
                            .lock_focus(true),
                    );
                    r.request_focus();
                    ui.separator();
                    let suggestions = self.palette.suggestions();
                    let sel = self.palette.selected.min(suggestions.len().saturating_sub(1));
                    for (i, v) in suggestions.iter().enumerate().take(10) {
                        let highlighted = i == sel;
                        let bg = if highlighted {
                            egui::Color32::from_rgb(0x3a, 0x6a, 0x9c)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        let frame = egui::Frame::none().fill(bg).inner_margin(egui::Margin::same(4.0));
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!(":{}", v.name)).strong().monospace());
                                ui.label(egui::RichText::new(v.summary).weak());
                            });
                            if highlighted {
                                ui.label(egui::RichText::new(v.usage).monospace().weak().small());
                            }
                        });
                    }
                });
            });
        if sel_prev {
            self.palette.select_prev();
        }
        if sel_next {
            self.palette.select_next();
        }
        if submit {
            actions = self
                .palette
                .submit(self.api.clone(), &self.rt_handle, &self.log_capture);
        } else if close {
            self.palette.close();
        }
        if !actions.is_empty() {
            for a in actions {
                self.apply_palette_action_simple(a);
            }
        }
    }

    fn draw_help(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        let mut open = self.help_open;
        egui::Window::new("Help")
            .open(&mut open)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Keys").strong());
                egui::Grid::new("help-keys")
                    .spacing([16.0, 2.0])
                    .show(ui, |ui| {
                        for (k, v) in &[
                            ("1–9", "switch screen"),
                            ("Tab / Shift+Tab", "cycle screens"),
                            (": / Ctrl+P", "open command palette"),
                            ("Ctrl+L", "toggle the bottom log pane (7 tabs)"),
                            ("Ctrl+A", "toggle alerts panel"),
                            ("Ctrl+N", "open node picker (switch active node)"),
                            ("?", "this help"),
                            ("↑ ↓ / j k", "move selection in the active list"),
                            ("Enter / click", "drill into the selected row"),
                            ("PgUp / PgDn", "page selection ±10 rows"),
                            ("Home / End", "first / last row"),
                            ("r", "re-poll fleet (S15) · run rchash bench (S4)"),
                            ("c", "check all pins (S10)"),
                            ("s", "cycle pin sort mode (S10)"),
                            ("Esc", "close any overlay or drill"),
                        ] {
                            ui.label(egui::RichText::new(*k).monospace());
                            ui.label(*v);
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.label(egui::RichText::new("Verbs").strong());
                for v in palette::VERBS {
                    ui.label(egui::RichText::new(format!(":{}", v.name)).monospace().strong());
                    ui.label(egui::RichText::new(v.summary).weak().small());
                    ui.label(egui::RichText::new(v.usage).monospace().small());
                    ui.add_space(4.0);
                }
            });
        self.help_open = open;
    }

    fn draw_node_picker(&mut self, ctx: &egui::Context) {
        if !self.node_picker_open {
            return;
        }
        if self.nodes.is_empty() {
            self.node_picker_open = false;
            return;
        }
        let n = self.nodes.len();
        if self.node_picker_sel >= n {
            self.node_picker_sel = 0;
        }
        let mut close = false;
        let mut submit = false;
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                close = true;
            }
            if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K) {
                self.node_picker_sel = (self.node_picker_sel + n - 1) % n;
            }
            if i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J) {
                self.node_picker_sel = (self.node_picker_sel + 1) % n;
            }
            if i.key_pressed(egui::Key::Enter) {
                submit = true;
            }
        });
        let mut clicked_name: Option<String> = None;
        let mut open = self.node_picker_open;
        egui::Window::new("Switch node")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("↑ ↓ to select  ·  Enter to switch  ·  Esc to cancel")
                        .weak()
                        .small(),
                );
                ui.add_space(4.0);
                for (i, node) in self.nodes.iter().enumerate() {
                    let is_sel = i == self.node_picker_sel;
                    let is_active = node.name == self.active_name;
                    let bg = if is_sel {
                        ui.style().visuals.selection.bg_fill.linear_multiply(0.5)
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let frame = egui::Frame::none()
                        .fill(bg)
                        .inner_margin(egui::Margin::symmetric(6.0, 4.0));
                    let resp = frame
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let marker = if is_active { "●" } else { "○" };
                                let marker_color = if is_active {
                                    egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)
                                } else {
                                    egui::Color32::GRAY
                                };
                                ui.label(egui::RichText::new(marker).color(marker_color));
                                ui.label(egui::RichText::new(&node.name).strong());
                                ui.label(egui::RichText::new(&node.url).monospace().weak());
                            });
                        })
                        .response
                        .interact(egui::Sense::click());
                    if resp.clicked() {
                        self.node_picker_sel = i;
                        clicked_name = Some(node.name.clone());
                    }
                }
            });
        self.node_picker_open = open;
        if close {
            self.node_picker_open = false;
            return;
        }
        let target = if submit {
            self.nodes.get(self.node_picker_sel).map(|n| n.name.clone())
        } else {
            clicked_name
        };
        if let Some(name) = target {
            self.node_picker_open = false;
            self.switch_active_node(&name);
        }
    }

    fn apply_palette_action_simple(&mut self, a: palette::PaletteAction) {
        match a {
            palette::PaletteAction::SwitchScreen(s) => self.screen = s,
            palette::PaletteAction::ToggleLogs => self.log_pane_open = !self.log_pane_open,
            palette::PaletteAction::ToggleAlerts => {
                self.alerts_open = !self.alerts_open;
                if self.alerts_open {
                    self.alerts.mark_read();
                }
            }
            palette::PaletteAction::ShowHelp => self.help_open = true,
            // Request a graceful close via the viewport command queue
            // rather than std::process::exit — eframe's normal close
            // path then runs on_exit, which is where the supervised
            // Bee gets a clean SIGTERM. Bypassing on_exit would leave
            // Bee to be killed by Drop's SIGKILL, skipping the
            // RocksDB-safe shutdown grace.
            palette::PaletteAction::Quit => self.pending_quit = true,
            palette::PaletteAction::LoadManifest(r) => {
                self.state.manifest.load_external(r, &self.api, &self.rt_handle);
            }
            palette::PaletteAction::LoadFeedTimeline { owner, topic, max } => {
                self.state
                    .feed_timeline
                    .load_external(owner, topic, max, &self.api, &self.rt_handle);
            }
            palette::PaletteAction::WatchlistAdd(r) => {
                self.state.watchlist.add_external(r, &self.api, &self.rt_handle);
            }
            palette::PaletteAction::OpenNodePicker => {
                self.node_picker_open = true;
                self.node_picker_sel = self
                    .nodes
                    .iter()
                    .position(|n| n.name == self.active_name)
                    .unwrap_or(0);
            }
            palette::PaletteAction::SwitchContext(name) => {
                self.switch_active_node(&name);
            }
        }
    }

    fn apply_palette_action(&mut self, a: palette::PaletteAction, _frame: &mut eframe::Frame) {
        self.apply_palette_action_simple(a);
    }

    fn connection_dot(&self) -> egui::Color32 {
        let rx = self.watch.health();
        let health = rx.borrow();
        if health.last_ping.is_some() {
            egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)
        } else if health.last_update.is_some() {
            egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)
        } else {
            egui::Color32::GRAY
        }
    }
}

fn draw_alert(ui: &mut egui::Ui, ta: &alerts::TimestampedAlert) {
    use bee_cockpit_core::views::health::GateStatus;
    let secs = ta
        .when
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(secs);
    let age = now.saturating_sub(secs);
    let color = match ta.alert.to {
        GateStatus::Fail => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        GateStatus::Warn => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        GateStatus::Pass => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        GateStatus::Unknown => egui::Color32::GRAY,
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format_age(age)).monospace().weak());
        ui.label(egui::RichText::new(&ta.alert.gate).strong());
        ui.label(
            egui::RichText::new(format!("{:?} → {:?}", ta.alert.from, ta.alert.to)).color(color),
        );
        ui.label(egui::RichText::new(&ta.alert.value).monospace());
    });
    if let Some(why) = &ta.alert.why {
        ui.label(egui::RichText::new(why).italics().weak().small());
    }
    ui.separator();
}

fn supervisor_chip(status: &BeeStatus) -> (String, egui::Color32) {
    match status {
        BeeStatus::Running => (
            "● bee running".into(),
            egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        ),
        BeeStatus::Exited(0) => (
            "○ bee exited (0)".into(),
            egui::Color32::GRAY,
        ),
        BeeStatus::Exited(code) => (
            format!("✕ bee exited ({code})"),
            egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        ),
        BeeStatus::Signaled(sig) => (
            format!("✕ bee killed (sig {sig})"),
            egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        ),
        BeeStatus::UnknownExit(msg) => (
            format!("✕ bee: {msg}"),
            egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        ),
    }
}

fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

fn draw_log_pane(
    ui: &mut egui::Ui,
    capture: &LogCapture,
    bee_logs: &bee_log::BeeLogs,
    tab: &mut bee_cockpit_core::bee_log::LogTab,
) {
    use bee_cockpit_core::bee_log::LogTab;
    let source_label = match &bee_logs.source {
        bee_log::ResolvedSource::None { .. } => String::from("(no bee-log source)"),
        bee_log::ResolvedSource::File { path, origin } => {
            format!("file: {} [{}]", path.display(), origin.label())
        }
        bee_log::ResolvedSource::Command { command, origin } => {
            format!("cmd: {command} [{}]", origin.label())
        }
    };
    ui.horizontal_wrapped(|ui| {
        for t in LogTab::ALL.iter().copied() {
            let count = tab_entry_count(t, capture, bee_logs);
            let label = if count > 0 {
                format!("{} ({count})", t.label())
            } else {
                t.label().to_string()
            };
            if ui.selectable_label(*tab == t, label).clicked() {
                *tab = t;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(source_label).weak().small());
        });
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false; 2])
        .show(ui, |ui| match *tab {
            LogTab::SelfHttp => draw_self_http_tab(ui, capture),
            LogTab::Cockpit => draw_cockpit_tab(ui),
            other => draw_bee_log_tab(ui, bee_logs, other),
        });
}

fn tab_entry_count(
    tab: bee_cockpit_core::bee_log::LogTab,
    capture: &LogCapture,
    bee_logs: &bee_log::BeeLogs,
) -> usize {
    use bee_cockpit_core::bee_log::LogTab;
    match tab {
        LogTab::SelfHttp => capture.snapshot().len(),
        LogTab::Cockpit => bee_cockpit_core::log_capture::cockpit_handle()
            .map(|h| h.snapshot().len())
            .unwrap_or(0),
        other => bee_logs.snapshot(other).len(),
    }
}

fn draw_self_http_tab(ui: &mut egui::Ui, capture: &LogCapture) {
    for entry in capture.snapshot().iter() {
        let status_color = match entry.status {
            Some(s) if s >= 500 => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
            Some(s) if s >= 400 => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
            Some(_) => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
            None => egui::Color32::GRAY,
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&entry.ts).monospace().weak());
            ui.label(
                egui::RichText::new(
                    entry
                        .status
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "—".into()),
                )
                .color(status_color)
                .monospace(),
            );
            ui.label(egui::RichText::new(&entry.method).monospace());
            ui.label(
                egui::RichText::new(
                    entry
                        .elapsed_ms
                        .map(|m| format!("{m}ms"))
                        .unwrap_or_else(|| "—".into()),
                )
                .monospace()
                .weak(),
            );
            ui.label(egui::RichText::new(&entry.url).monospace());
        });
    }
}

fn draw_cockpit_tab(ui: &mut egui::Ui) {
    let Some(handle) = bee_cockpit_core::log_capture::cockpit_handle() else {
        ui.label(
            egui::RichText::new("(cockpit capture not installed)")
                .italics()
                .weak(),
        );
        return;
    };
    for entry in handle.snapshot().iter() {
        let level_color = match entry.level.as_str() {
            "ERROR" => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
            "WARN" => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
            "INFO" => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
            "DEBUG" => egui::Color32::GRAY,
            _ => egui::Color32::GRAY,
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&entry.ts).monospace().weak());
            ui.label(
                egui::RichText::new(&entry.level)
                    .color(level_color)
                    .monospace(),
            );
            ui.label(egui::RichText::new(&entry.target).monospace().weak());
            ui.label(egui::RichText::new(&entry.message).monospace());
        });
    }
}

fn draw_bee_log_tab(
    ui: &mut egui::Ui,
    bee_logs: &bee_log::BeeLogs,
    tab: bee_cockpit_core::bee_log::LogTab,
) {
    let entries = bee_logs.snapshot(tab);
    if entries.is_empty() {
        let hint = match &bee_logs.source {
            bee_log::ResolvedSource::None { reason } => reason.clone(),
            _ => String::from("(no entries yet on this tab)"),
        };
        ui.label(egui::RichText::new(hint).italics().weak().small());
        return;
    }
    for line in entries.iter() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&line.timestamp).monospace().weak());
            ui.label(egui::RichText::new(&line.logger).monospace().weak());
            ui.label(egui::RichText::new(&line.message).monospace());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_parse_accepts_canonical_names() {
        assert!(matches!(Theme::parse("auto"), Some(Theme::Auto)));
        assert!(matches!(Theme::parse("light"), Some(Theme::Light)));
        assert!(matches!(Theme::parse("dark"), Some(Theme::Dark)));
    }

    #[test]
    fn theme_parse_case_insensitive_and_aliases() {
        assert!(matches!(Theme::parse("AUTO"), Some(Theme::Auto)));
        assert!(matches!(Theme::parse("Light"), Some(Theme::Light)));
        assert!(matches!(Theme::parse("default"), Some(Theme::Auto)));
        assert!(matches!(Theme::parse("mono"), Some(Theme::Dark)));
    }

    #[test]
    fn theme_parse_rejects_unknown() {
        assert!(Theme::parse("rainbow").is_none());
        assert!(Theme::parse("").is_none());
    }

    #[test]
    fn format_age_buckets_seconds_minutes_hours() {
        assert_eq!(format_age(0), "0s ago");
        assert_eq!(format_age(59), "59s ago");
        assert_eq!(format_age(60), "1m ago");
        assert_eq!(format_age(3599), "59m ago");
        assert_eq!(format_age(3600), "1h ago");
        assert_eq!(format_age(7200), "2h ago");
    }

    #[test]
    fn supervisor_chip_running_is_green() {
        let (label, color) = supervisor_chip(&BeeStatus::Running);
        assert!(label.contains("running"));
        assert_eq!(color, egui::Color32::from_rgb(0x4a, 0xc0, 0x4a));
    }

    #[test]
    fn supervisor_chip_clean_exit_is_grey() {
        let (label, color) = supervisor_chip(&BeeStatus::Exited(0));
        assert!(label.contains("0"));
        assert_eq!(color, egui::Color32::GRAY);
    }

    #[test]
    fn supervisor_chip_nonzero_exit_is_red() {
        let (label, color) = supervisor_chip(&BeeStatus::Exited(137));
        assert!(label.contains("137"));
        assert_eq!(color, egui::Color32::from_rgb(0xd0, 0x4a, 0x4a));
    }

    #[test]
    fn supervisor_chip_signaled_is_red() {
        let (label, color) = supervisor_chip(&BeeStatus::Signaled(9));
        assert!(label.contains("9"));
        assert_eq!(color, egui::Color32::from_rgb(0xd0, 0x4a, 0x4a));
    }

    #[test]
    fn supervisor_chip_unknown_exit_is_red() {
        let (label, color) =
            supervisor_chip(&BeeStatus::UnknownExit("waitpid: ECHILD".into()));
        assert!(label.contains("waitpid"));
        assert_eq!(color, egui::Color32::from_rgb(0xd0, 0x4a, 0x4a));
    }
}
