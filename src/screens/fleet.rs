//! S15 Fleet screen. Subscribes to the [`bee_cockpit_core::fleet`]
//! poller's [`FleetSnapshot`] receiver and renders the aggregate
//! table via [`bee_cockpit_core::views::fleet::view_for`].

use bee_cockpit_core::fleet::{FleetSnapshot, FleetStatus};
use bee_cockpit_core::views::fleet::{FleetRowView, FleetView, view_for};
use tokio::sync::watch;

fn status_color(s: FleetStatus) -> egui::Color32 {
    match s {
        FleetStatus::Pass => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        FleetStatus::Warn => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        FleetStatus::Fail => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        FleetStatus::Unknown => egui::Color32::GRAY,
    }
}

pub fn draw(ui: &mut egui::Ui, rx: Option<&watch::Receiver<FleetSnapshot>>, active_name: &str) {
    let Some(rx) = rx else {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.heading("Fleet");
            ui.label(
                egui::RichText::new("single-node mode — pass multiple node URLs or list them in config to aggregate.")
                    .italics()
                    .weak(),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("example: beegui http://a:1633 http://b:1633")
                    .monospace()
                    .weak()
                    .small(),
            );
        });
        return;
    };
    let snap = rx.borrow().clone();
    let view = view_for(&snap, active_name, 0);

    draw_header(ui, &view);
    ui.add_space(8.0);
    draw_rows(ui, &view.rows);
}

fn draw_header(ui: &mut egui::Ui, view: &FleetView) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("total {}", view.header.total)).strong());
        ui.label(
            egui::RichText::new(format!("pass {}", view.header.pass))
                .color(status_color(FleetStatus::Pass)),
        );
        ui.label(
            egui::RichText::new(format!("warn {}", view.header.warn))
                .color(status_color(FleetStatus::Warn)),
        );
        ui.label(
            egui::RichText::new(format!("fail {}", view.header.fail))
                .color(status_color(FleetStatus::Fail)),
        );
        ui.label(egui::RichText::new(format!("unknown {}", view.header.unknown)).weak());
    });
}

fn draw_rows(ui: &mut egui::Ui, rows: &[FleetRowView]) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("fleet")
            .num_columns(6)
            .spacing([14.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("status").strong());
                ui.label(egui::RichText::new("node").strong());
                ui.label(egui::RichText::new("url").strong());
                ui.label(egui::RichText::new("peers").strong());
                ui.label(egui::RichText::new("worst TTL").strong());
                ui.label(egui::RichText::new("ping").strong());
                ui.end_row();
                for row in rows {
                    draw_row(ui, row);
                    ui.end_row();
                }
            });
    });
}

fn draw_row(ui: &mut egui::Ui, row: &FleetRowView) {
    ui.label(egui::RichText::new(&row.status_label).color(status_color(row.status)));
    let name = if row.active {
        egui::RichText::new(&row.name).strong()
    } else if row.default {
        egui::RichText::new(format!("{} *", row.name))
    } else {
        egui::RichText::new(&row.name)
    };
    ui.label(name);
    ui.label(egui::RichText::new(&row.url).monospace().weak());
    ui.label(egui::RichText::new(&row.peers_label).monospace());
    ui.label(egui::RichText::new(&row.ttl_label).monospace());
    if let Some(why) = &row.why {
        ui.label(
            egui::RichText::new(format!("{} · {}", row.ping_label, why))
                .color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a))
                .monospace(),
        );
    } else {
        ui.label(egui::RichText::new(&row.ping_label).monospace());
    }
}
