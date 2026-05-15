//! S10 Pins screen. Pin-check pipeline state is renderer-local;
//! beegui's first cut just lists the pins from the watch without
//! issuing background checks (that lands with the durability worker
//! in Phase 3).

use std::collections::HashMap;

use bee::swarm::Reference;
use bee_cockpit_core::views::pins::{CheckState, PinRow, SortMode, view_for};
use bee_cockpit_core::watch::BeeWatch;

#[derive(Default)]
pub struct PinsScreenState {
    selected: usize,
}

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch, state: &mut PinsScreenState) {
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

    let n = view.rows.len();
    if !ui.ctx().memory(|m| m.focused().is_some()) {
        ui.input(|i| {
            if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K) {
                state.selected = state.selected.saturating_sub(1);
            }
            if (i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J))
                && state.selected + 1 < n
            {
                state.selected += 1;
            }
        });
    }
    if state.selected >= n.max(1) {
        state.selected = n.saturating_sub(1);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, row) in view.rows.iter().enumerate() {
            let resp = draw_row(ui, row, i == state.selected);
            if resp.clicked() {
                state.selected = i;
            }
        }
    });
}

fn draw_row(ui: &mut egui::Ui, row: &PinRow, selected: bool) -> egui::Response {
    let bg = if selected {
        egui::Color32::from_rgb(0x3a, 0x6a, 0x9c)
    } else {
        egui::Color32::TRANSPARENT
    };
    let mut frame = egui::Frame::none().fill(bg);
    frame.inner_margin = egui::Margin::symmetric(4.0, 1.0);
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
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&row.reference_short).monospace());
                ui.label(egui::RichText::new(label).color(color));
            });
        })
        .response;
    resp.interact(egui::Sense::click())
}
