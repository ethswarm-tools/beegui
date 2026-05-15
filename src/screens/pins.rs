//! S10 Pins screen. Pin-check pipeline state is renderer-local;
//! beegui's first cut just lists the pins from the watch without
//! issuing background checks (that lands with the durability worker
//! in Phase 3).

use std::collections::HashMap;

use bee::swarm::Reference;
use bee_cockpit_core::views::pins::{CheckState, PinRow, SortMode, view_for};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch) {
    let snap = watch.pins().borrow().clone();
    let checks: HashMap<Reference, CheckState> = HashMap::new();
    let view = view_for(&snap, &checks, SortMode::Reference);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("total {}", view.total_pins)).strong());
        ui.label(
            egui::RichText::new(format!("healthy {}", view.healthy))
                .color(egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)),
        );
        ui.label(
            egui::RichText::new(format!("unhealthy {}", view.unhealthy))
                .color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)),
        );
        ui.label(egui::RichText::new(format!("unchecked {}", view.unchecked)).weak());
    });
    ui.add_space(8.0);

    if view.rows.is_empty() {
        ui.label(egui::RichText::new("(no pins)").italics().weak());
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("pins")
            .num_columns(2)
            .spacing([14.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("reference").strong());
                ui.label(egui::RichText::new("check").strong());
                ui.end_row();
                for row in &view.rows {
                    draw_row(ui, row);
                    ui.end_row();
                }
            });
    });
}

fn draw_row(ui: &mut egui::Ui, row: &PinRow) {
    ui.label(egui::RichText::new(&row.reference_short).monospace());
    let (label, color) = match &row.check {
        CheckState::Idle => ("unchecked", egui::Color32::GRAY),
        CheckState::Checking => ("checking…", egui::Color32::from_rgb(0xe0, 0xb0, 0x30)),
        CheckState::Ok { total, missing, .. } if *missing == 0 => (
            "ok",
            if *total > 0 {
                egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)
            } else {
                egui::Color32::GRAY
            },
        ),
        CheckState::Ok { .. } => ("unhealthy", egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)),
        CheckState::Failed(_) => ("error", egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)),
    };
    ui.label(egui::RichText::new(label).color(color));
}
