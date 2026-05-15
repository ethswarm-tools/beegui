//! S2 Stamps screen. Renders [`bee_cockpit_core::views::stamps::rows_for`]
//! as a table.

use bee_cockpit_core::views::stamps::{StampRow, StampStatus, rows_for};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch) {
    let snap = watch.stamps().borrow().clone();
    let rows = rows_for(&snap);

    if rows.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            if snap.last_update.is_none() {
                ui.label(egui::RichText::new("loading…").italics().weak());
            } else if let Some(err) = &snap.last_error {
                ui.label(egui::RichText::new(format!("error: {err}")).color(egui::Color32::RED));
            } else {
                ui.label(egui::RichText::new("no postage batches").italics().weak());
            }
        });
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("stamps")
            .num_columns(7)
            .spacing([16.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("status").strong());
                ui.label(egui::RichText::new("label").strong());
                ui.label(egui::RichText::new("batch id").strong());
                ui.label(egui::RichText::new("volume").strong());
                ui.label(egui::RichText::new("worst bucket").strong());
                ui.label(egui::RichText::new("TTL").strong());
                ui.label(egui::RichText::new("type").strong());
                ui.end_row();

                for row in &rows {
                    draw_row(ui, row);
                    ui.end_row();
                }
            });
    });
}

fn draw_row(ui: &mut egui::Ui, row: &StampRow) {
    ui.label(egui::RichText::new(row.status.label()).color(status_color(row.status)));
    ui.label(&row.label);
    ui.label(egui::RichText::new(&row.batch_id_short).monospace().weak());
    ui.label(egui::RichText::new(&row.volume).monospace());
    ui.label(
        egui::RichText::new(format!(
            "{:>3}% ({})",
            row.worst_bucket_pct, row.worst_bucket_raw
        ))
        .monospace(),
    );
    ui.label(egui::RichText::new(&row.ttl).monospace());
    ui.label(if row.immutable { "immutable" } else { "mutable" });
}

fn status_color(s: StampStatus) -> egui::Color32 {
    match s {
        StampStatus::Healthy => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        StampStatus::Skewed => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        StampStatus::Critical | StampStatus::Expired => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        StampStatus::Pending => egui::Color32::GRAY,
    }
}
