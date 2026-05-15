//! beegui — desktop GUI cockpit for Ethereum Swarm Bee node operators.
//!
//! Sibling of [bee-tui]. The cockpit logic — health gates, stamp
//! warnings, fleet roll-up, redistribution skip reasons — lives in
//! [bee-cockpit-core]; this crate just renders it with [egui] instead
//! of ratatui.
//!
//! v0.1: scaffold + first screen (S1 Health). Boots a tokio runtime
//! alongside eframe, starts a [`BeeWatch`] hub against
//! `$BEE_NODE_URL` (default `http://localhost:1633`), and renders the
//! same gate list bee-tui's S1 produces — via the same
//! `gates_for_with_stamps` function from core.
//!
//! [bee-tui]: https://github.com/ethswarm-tools/bee-tui
//! [bee-cockpit-core]: https://github.com/ethswarm-tools/bee-cockpit-core
//! [egui]: https://github.com/emilk/egui

use std::sync::Arc;

use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::config::NodeConfig;
use bee_cockpit_core::views::health::{Gate, GateStatus, gates_for_with_stamps};
use bee_cockpit_core::watch::BeeWatch;
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

const DEFAULT_URL: &str = "http://localhost:1633";

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let url = std::env::var("BEE_NODE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());

    let runtime = Runtime::new()?;
    let cancel = CancellationToken::new();
    let watch = {
        let node = NodeConfig {
            name: "default".into(),
            url: url.clone(),
            token: std::env::var("BEE_NODE_TOKEN").ok(),
            log_file: None,
            log_command: None,
            default: true,
        };
        let client = Arc::new(ApiClient::from_node(&node)?);
        let _guard = runtime.enter();
        BeeWatch::start(client, &cancel)
    };

    let app = App {
        url,
        watch,
        _runtime: runtime,
        _cancel: cancel,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native("beegui", options, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| color_eyre::eyre::eyre!("eframe: {e}"))
}

struct App {
    url: String,
    watch: BeeWatch,
    _runtime: Runtime,
    _cancel: CancellationToken,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        let health = self.watch.health().borrow().clone();
        let topology = self.watch.topology().borrow().clone();
        let stamps = self.watch.stamps().borrow().clone();
        let gates = gates_for_with_stamps(&health, Some(&topology), Some(&stamps));

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("beegui · S1 Health");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.url).monospace().weak());
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Grid::new("gates")
                .num_columns(3)
                .spacing([16.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    for gate in &gates {
                        draw_gate(ui, gate);
                        ui.end_row();
                    }
                });
        });
    }
}

fn draw_gate(ui: &mut egui::Ui, gate: &Gate) {
    ui.label(egui::RichText::new(status_glyph(gate.status)).color(status_color(gate.status)));
    ui.label(egui::RichText::new(gate.label).strong());
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(&gate.value).monospace());
        if let Some(why) = &gate.why {
            ui.label(egui::RichText::new(why).italics().weak().small());
        }
    });
}

fn status_glyph(s: GateStatus) -> &'static str {
    match s {
        GateStatus::Pass => "✔",
        GateStatus::Warn => "!",
        GateStatus::Fail => "✘",
        GateStatus::Unknown => "·",
    }
}

fn status_color(s: GateStatus) -> egui::Color32 {
    match s {
        GateStatus::Pass => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        GateStatus::Warn => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        GateStatus::Fail => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        GateStatus::Unknown => egui::Color32::GRAY,
    }
}
