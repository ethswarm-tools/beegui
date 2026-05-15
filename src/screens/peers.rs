//! S6 Peers screen. Renders [`bee_cockpit_core::views::peers::view_for`].

use bee_cockpit_core::views::peers::{BinSaturation, BinStripRow, PeerRow, PeersView, view_for};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch) {
    let topology = watch.topology().borrow().clone();
    let Some(view) = view_for(&topology) else {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.label(
                egui::RichText::new("topology not yet loaded")
                    .italics()
                    .weak(),
            );
            if let Some(err) = &topology.last_error {
                ui.label(egui::RichText::new(err).color(egui::Color32::RED));
            }
        });
        return;
    };

    draw_header(ui, &view);
    ui.add_space(8.0);
    draw_bins(ui, &view.bins);
    ui.add_space(8.0);
    draw_peers(ui, &view.peers);
}

fn draw_header(ui: &mut egui::Ui, view: &PeersView) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("depth {}", view.depth)).strong());
        ui.label(egui::RichText::new(format!("connected {}", view.connected)).monospace());
        ui.label(egui::RichText::new(format!("population {}", view.population)).monospace());
        ui.label(egui::RichText::new(format!("light {}", view.light_connected)).monospace());
        ui.label(
            egui::RichText::new(format!(
                "starving {} · over {}",
                view.saturation.starving, view.saturation.over
            ))
            .monospace(),
        );
        if !view.reachability.is_empty() {
            ui.label(egui::RichText::new(&view.reachability).weak());
        }
    });
}

fn draw_bins(ui: &mut egui::Ui, bins: &[BinStripRow]) {
    ui.label(egui::RichText::new("Bins").strong());
    egui::Grid::new("bins")
        .num_columns(4)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("bin").strong());
            ui.label(egui::RichText::new("connected").strong());
            ui.label(egui::RichText::new("population").strong());
            ui.label(egui::RichText::new("status").strong());
            ui.end_row();
            for b in bins {
                let text = egui::RichText::new(format!("{:>2}", b.bin)).monospace();
                ui.label(if b.is_relevant { text.strong() } else { text });
                ui.label(egui::RichText::new(b.connected.to_string()).monospace());
                ui.label(egui::RichText::new(b.population.to_string()).monospace());
                ui.label(
                    egui::RichText::new(saturation_label(b.status)).color(saturation_color(b.status)),
                );
                ui.end_row();
            }
        });
}

fn draw_peers(ui: &mut egui::Ui, peers: &[PeerRow]) {
    ui.label(egui::RichText::new(format!("Peers ({})", peers.len())).strong());
    egui::ScrollArea::vertical()
        .id_salt("peers")
        .show(ui, |ui| {
            egui::Grid::new("peer-rows")
                .num_columns(5)
                .spacing([12.0, 2.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("bin").strong());
                    ui.label(egui::RichText::new("overlay").strong());
                    ui.label(egui::RichText::new("dir").strong());
                    ui.label(egui::RichText::new("ping").strong());
                    ui.label(egui::RichText::new("reach").strong());
                    ui.end_row();
                    for p in peers {
                        ui.label(egui::RichText::new(p.bin.to_string()).monospace());
                        ui.label(egui::RichText::new(&p.peer_short).monospace());
                        ui.label(p.direction);
                        let lat = egui::RichText::new(&p.latency).monospace();
                        ui.label(if p.healthy {
                            lat
                        } else {
                            lat.color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a))
                        });
                        ui.label(egui::RichText::new(&p.reachability).weak());
                        ui.end_row();
                    }
                });
        });
}

fn saturation_label(s: BinSaturation) -> &'static str {
    match s {
        BinSaturation::Empty => "empty",
        BinSaturation::Starving => "starving",
        BinSaturation::Healthy => "healthy",
        BinSaturation::Over => "over",
    }
}

fn saturation_color(s: BinSaturation) -> egui::Color32 {
    match s {
        BinSaturation::Empty => egui::Color32::DARK_GRAY,
        BinSaturation::Starving => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        BinSaturation::Healthy => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        BinSaturation::Over => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
    }
}
