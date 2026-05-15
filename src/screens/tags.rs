//! S9 Tags screen.

use bee_cockpit_core::views::tags::{TagRow, TagStatus, view_for};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch) {
    let snap = watch.tags().borrow().clone();
    let view = view_for(&snap);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("tags {}", view.totals.tags)).strong());
        ui.label(egui::RichText::new(format!("active {}", view.totals.active)).monospace());
        ui.label(egui::RichText::new(format!("split {}", view.totals.split)).monospace());
        ui.label(egui::RichText::new(format!("sent {}", view.totals.sent)).monospace());
        ui.label(egui::RichText::new(format!("synced {}", view.totals.synced)).monospace());
    });
    ui.add_space(8.0);

    if view.rows.is_empty() {
        ui.label(egui::RichText::new("(no tags)").italics().weak());
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("tags")
            .num_columns(6)
            .spacing([14.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("status").strong());
                ui.label(egui::RichText::new("uid").strong());
                ui.label(egui::RichText::new("name").strong());
                ui.label(egui::RichText::new("progress").strong());
                ui.label(egui::RichText::new("counts").strong());
                ui.label(egui::RichText::new("ref").strong());
                ui.end_row();
                for row in &view.rows {
                    draw_row(ui, row);
                    ui.end_row();
                }
            });
    });
}

fn draw_row(ui: &mut egui::Ui, row: &TagRow) {
    ui.label(egui::RichText::new(status_label(row.status)).color(status_color(row.status)));
    ui.label(egui::RichText::new(row.uid.to_string()).monospace());
    ui.label(&row.name);
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(format!("{}%", row.completion_pct)).monospace());
        ui.add(egui::ProgressBar::new(row.completion_pct as f32 / 100.0).desired_width(120.0));
    });
    ui.label(
        egui::RichText::new(format!(
            "split {} · sent {} · synced {} / {}",
            row.split, row.sent, row.synced, row.total
        ))
        .monospace(),
    );
    ui.label(egui::RichText::new(&row.address_short).monospace().weak());
}

fn status_label(s: TagStatus) -> &'static str {
    match s {
        TagStatus::Pending => "pending",
        TagStatus::Splitting => "splitting",
        TagStatus::Pushing => "pushing",
        TagStatus::Syncing => "syncing",
        TagStatus::Synced => "synced",
    }
}

fn status_color(s: TagStatus) -> egui::Color32 {
    match s {
        TagStatus::Synced => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        TagStatus::Syncing | TagStatus::Pushing => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        TagStatus::Splitting => egui::Color32::from_rgb(0x4a, 0x9c, 0xe0),
        TagStatus::Pending => egui::Color32::GRAY,
    }
}
