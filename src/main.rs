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
mod screens;

use std::path::PathBuf;
use std::sync::Arc;

use std::time::Duration;

use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::config::{
    Config, ConfigPaths, NodeConfig, load_raw, nodes_from_urls, normalize_url,
};
use bee_cockpit_core::fleet::{FleetSnapshot, spawn_poller};
use bee_cockpit_core::alerts::DEFAULT_DEBOUNCE_SECS;
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

const PATHS: ConfigPaths = ConfigPaths {
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
    /// Ad-hoc node URLs (positional). When supplied, beegui ignores
    /// the config's node list and uses these instead. Mirrors
    /// bee-tui's positional-URL fleet flow.
    urls: Vec<String>,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let log_capture = init_logging();
    let resolved = resolve_nodes(&cli)?;

    let runtime = Runtime::new()?;
    let cancel = CancellationToken::new();
    let active = resolved.active.clone();
    let url = active.url.clone();
    let api = Arc::new(ApiClient::from_node(&active)?);
    let watch = {
        let _guard = runtime.enter();
        BeeWatch::start(api.clone(), &cancel)
    };
    let rt_handle = runtime.handle().clone();
    let fleet_rx = if resolved.all.len() > 1 {
        let _guard = runtime.enter();
        let (rx, _resync) = spawn_poller(
            resolved.all.clone(),
            cancel.child_token(),
            FLEET_POLL_INTERVAL,
        );
        Some(rx)
    } else {
        None
    };

    let app = App {
        url,
        active_name: active.name,
        api,
        rt_handle,
        watch,
        fleet_rx,
        log_capture,
        log_pane_open: false,
        alerts: alerts::AlertsPipeline::new(DEFAULT_DEBOUNCE_SECS),
        alerts_open: false,
        screen: Screen::Health,
        state: ScreenState::default(),
        _runtime: runtime,
        _cancel: cancel,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native("beegui", options, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| color_eyre::eyre::eyre!("eframe: {e}"))
}

struct ResolvedNodes {
    active: NodeConfig,
    all: Vec<NodeConfig>,
}

fn resolve_nodes(cli: &Cli) -> color_eyre::Result<ResolvedNodes> {
    let token_override = cli
        .token
        .clone()
        .or_else(|| std::env::var("BEE_NODE_TOKEN").ok());

    if let Some(url) = &cli.node {
        let node = NodeConfig {
            name: "cli".into(),
            url: normalize_url(url),
            token: token_override,
            log_file: None,
            log_command: None,
            default: true,
        };
        return Ok(ResolvedNodes {
            active: node.clone(),
            all: vec![node],
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
        return Ok(ResolvedNodes { active, all });
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
        return Ok(ResolvedNodes {
            active: node.clone(),
            all: vec![node],
        });
    }

    match load_raw::<Config>(&PATHS, cli.config.as_deref()) {
        Ok(cfg) => {
            if !cfg.nodes.is_empty() {
                let mut all = cfg.nodes.clone();
                if let Some(t) = token_override.clone() {
                    for n in &mut all {
                        if n.token.is_none() {
                            n.token = Some(t.clone());
                        }
                    }
                }
                let active = cfg
                    .active_node()
                    .cloned()
                    .unwrap_or_else(|| all[0].clone());
                return Ok(ResolvedNodes { active, all });
            }
        }
        Err(e) => {
            if cli.config.is_some() {
                return Err(color_eyre::eyre::eyre!("config: {e}"));
            }
        }
    }

    let node = NodeConfig {
        name: "default".into(),
        url: DEFAULT_URL.to_string(),
        token: token_override,
        log_file: None,
        log_command: None,
        default: true,
    };
    Ok(ResolvedNodes {
        active: node.clone(),
        all: vec![node],
    })
}

struct App {
    url: String,
    active_name: String,
    api: Arc<ApiClient>,
    rt_handle: tokio::runtime::Handle,
    watch: BeeWatch,
    fleet_rx: Option<watch::Receiver<FleetSnapshot>>,
    log_capture: LogCapture,
    log_pane_open: bool,
    alerts: alerts::AlertsPipeline,
    alerts_open: bool,
    screen: Screen,
    state: ScreenState,
    _runtime: Runtime,
    _cancel: CancellationToken,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
        self.handle_keys(ctx);
        self.observe_alerts();

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
                    .on_hover_text("Toggle bee::http log pane (Ctrl+L)")
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
                });
            });
        });

        if self.log_pane_open {
            egui::TopBottomPanel::bottom("logs")
                .resizable(true)
                .default_height(180.0)
                .show(ctx, |ui| {
                    draw_log_pane(ui, &self.log_capture);
                });
        }

        if self.alerts_open {
            let mut clear = false;
            egui::Window::new("Alerts")
                .open(&mut self.alerts_open)
                .default_width(560.0)
                .default_height(360.0)
                .show(ctx, |ui| {
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

        egui::CentralPanel::default().show(ctx, |ui| {
            screens::draw(
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
                    log_capture: &self.log_capture,
                },
            );
        });
    }
}

impl App {
    fn handle_keys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            let screens = Screen::all();
            let idx = self.screen.index();
            if i.modifiers.ctrl && i.key_pressed(egui::Key::L) {
                self.log_pane_open = !self.log_pane_open;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::A) {
                self.alerts_open = !self.alerts_open;
                if self.alerts_open {
                    self.alerts.mark_read();
                }
            }
            if i.key_pressed(egui::Key::Tab) {
                let next = if i.modifiers.shift {
                    (idx + screens.len() - 1) % screens.len()
                } else {
                    (idx + 1) % screens.len()
                };
                if let Some(s) = Screen::from_index(next) {
                    self.screen = s;
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
                if i.key_pressed(key)
                    && let Some(s) = Screen::from_index(n - 1)
                {
                    self.screen = s;
                }
            }
        });
    }

    fn observe_alerts(&mut self) {
        let health = self.watch.health().borrow().clone();
        let topology = self.watch.topology().borrow().clone();
        let stamps = self.watch.stamps().borrow().clone();
        let gates = gates_for_with_stamps(&health, Some(&topology), Some(&stamps));
        self.alerts.observe(&gates);
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

fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

fn draw_log_pane(ui: &mut egui::Ui, capture: &LogCapture) {
    let entries = capture.snapshot();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("bee::http log").strong());
        ui.label(egui::RichText::new(format!("({} entries)", entries.len())).weak());
    });
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for entry in entries.iter() {
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
        });
}
