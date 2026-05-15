//! S1 Health screen. Renders the gate list from
//! [`bee_cockpit_core::views::health::gates_for_with_stamps`].

use bee_cockpit_core::views::health::{Gate, GateStatus, gates_for_with_stamps};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch) {
    let health = watch.health().borrow().clone();
    let topology = watch.topology().borrow().clone();
    let stamps = watch.stamps().borrow().clone();
    let gates = gates_for_with_stamps(&health, Some(&topology), Some(&stamps));

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
