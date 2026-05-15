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

mod screens;

use std::path::PathBuf;
use std::sync::Arc;

use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::config::{
    Config, ConfigPaths, NodeConfig, load_raw, nodes_from_urls, normalize_url,
};
use bee_cockpit_core::watch::BeeWatch;
use clap::Parser;
use screens::{Screen, ScreenState};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

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
    let node = resolve_node(&cli)?;

    let runtime = Runtime::new()?;
    let cancel = CancellationToken::new();
    let url = node.url.clone();
    let watch = {
        let client = Arc::new(ApiClient::from_node(&node)?);
        let _guard = runtime.enter();
        BeeWatch::start(client, &cancel)
    };

    let app = App {
        url,
        watch,
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

fn resolve_node(cli: &Cli) -> color_eyre::Result<NodeConfig> {
    if let Some(url) = &cli.node {
        return Ok(NodeConfig {
            name: "cli".into(),
            url: normalize_url(url),
            token: cli.token.clone().or_else(|| std::env::var("BEE_NODE_TOKEN").ok()),
            log_file: None,
            log_command: None,
            default: true,
        });
    }
    if !cli.urls.is_empty() {
        return Ok(nodes_from_urls(&cli.urls).into_iter().next().unwrap());
    }
    if let Ok(url) = std::env::var("BEE_NODE_URL") {
        return Ok(NodeConfig {
            name: "env".into(),
            url: normalize_url(&url),
            token: cli.token.clone().or_else(|| std::env::var("BEE_NODE_TOKEN").ok()),
            log_file: None,
            log_command: None,
            default: true,
        });
    }

    match load_raw::<Config>(&PATHS, cli.config.as_deref()) {
        Ok(cfg) => {
            if let Some(active) = cfg.active_node() {
                let mut node = active.clone();
                if let Some(t) = cli.token.clone() {
                    node.token = Some(t);
                }
                return Ok(node);
            }
        }
        Err(e) => {
            if cli.config.is_some() {
                return Err(color_eyre::eyre::eyre!("config: {e}"));
            }
        }
    }

    Ok(NodeConfig {
        name: "default".into(),
        url: DEFAULT_URL.to_string(),
        token: cli.token.clone().or_else(|| std::env::var("BEE_NODE_TOKEN").ok()),
        log_file: None,
        log_command: None,
        default: true,
    })
}

struct App {
    url: String,
    watch: BeeWatch,
    screen: Screen,
    state: ScreenState,
    _runtime: Runtime,
    _cancel: CancellationToken,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
        self.handle_keys(ctx);

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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("beegui {}", env!("CARGO_PKG_VERSION")))
                            .weak()
                            .small(),
                    );
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            screens::draw(self.screen, ui, &self.watch, &mut self.state, &self.url);
        });
    }
}

impl App {
    fn handle_keys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            let screens = Screen::all();
            let idx = self.screen.index();
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
