//! S9 Tags screen.

use bee_cockpit_core::views::tags::{TagRow, TagStatus, view_for};
use bee_cockpit_core::watch::BeeWatch;

#[derive(Default)]
pub struct TagsScreenState {
    selected: usize,
}

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch, state: &mut TagsScreenState) {
    let snap = watch.tags().borrow().clone();
    let view = view_for(&snap);
    arrow_nav(ui, &mut state.selected, view.rows.len());

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
        for (i, row) in view.rows.iter().enumerate() {
            let resp = draw_row(ui, row, i == state.selected);
            if resp.clicked() {
                state.selected = i;
            }
        }
    });
}

fn arrow_nav(ui: &mut egui::Ui, selected: &mut usize, n: usize) {
    if ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }
    ui.input(|i| {
        if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K) {
            *selected = selected.saturating_sub(1);
        }
        if (i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) && *selected + 1 < n
        {
            *selected += 1;
        }
        if i.key_pressed(egui::Key::PageUp) {
            *selected = selected.saturating_sub(10);
        }
        if i.key_pressed(egui::Key::PageDown) {
            *selected = (*selected + 10).min(n.saturating_sub(1));
        }
        if i.key_pressed(egui::Key::Home) {
            *selected = 0;
        }
        if i.key_pressed(egui::Key::End) && n > 0 {
            *selected = n - 1;
        }
    });
    if *selected >= n.max(1) {
        *selected = n.saturating_sub(1);
    }
}

fn draw_row(ui: &mut egui::Ui, row: &TagRow, selected: bool) -> egui::Response {
    let bg = if selected {
        egui::Color32::from_rgb(0x3a, 0x6a, 0x9c)
    } else {
        egui::Color32::TRANSPARENT
    };
    let mut frame = egui::Frame::none().fill(bg);
    frame.inner_margin = egui::Margin::symmetric(4.0, 1.0);
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(status_label(row.status))
                        .color(status_color(row.status))
                        .monospace(),
                );
                ui.label(egui::RichText::new(row.uid.to_string()).monospace());
                ui.label(egui::RichText::new(&row.name).monospace());
                ui.add(
                    egui::ProgressBar::new(row.completion_pct as f32 / 100.0)
                        .desired_width(120.0)
                        .text(format!("{}%", row.completion_pct)),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "split {} · sent {} · synced {} / {}",
                        row.split, row.sent, row.synced, row.total
                    ))
                    .monospace(),
                );
                ui.label(egui::RichText::new(&row.address_short).monospace().weak());
            });
        })
        .response;
    resp.interact(egui::Sense::click())
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
