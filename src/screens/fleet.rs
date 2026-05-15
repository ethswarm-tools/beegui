//! S15 Fleet screen. Subscribes to the [`bee_cockpit_core::fleet`]
//! poller's [`FleetSnapshot`] receiver and renders the aggregate
//! table via [`bee_cockpit_core::views::fleet::view_for`].

use bee_cockpit_core::fleet::{FleetSnapshot, FleetStatus};
use bee_cockpit_core::views::fleet::{FleetRowView, FleetView, view_for};
use tokio::sync::watch;

#[derive(Default)]
pub struct FleetScreenState {
    selected: usize,
}

fn status_color(s: FleetStatus) -> egui::Color32 {
    match s {
        FleetStatus::Pass => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        FleetStatus::Warn => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        FleetStatus::Fail => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        FleetStatus::Unknown => egui::Color32::GRAY,
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    rx: Option<&watch::Receiver<FleetSnapshot>>,
    active_name: &str,
    state: &mut FleetScreenState,
) {
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
    let view = view_for(&snap, active_name, state.selected);

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

    draw_header(ui, &view);
    ui.add_space(8.0);
    draw_rows(ui, &view.rows, &mut state.selected);
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

fn draw_rows(ui: &mut egui::Ui, rows: &[FleetRowView], selected: &mut usize) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, row) in rows.iter().enumerate() {
            let resp = draw_row(ui, row, i == *selected);
            if resp.clicked() {
                *selected = i;
            }
        }
    });
}

fn draw_row(ui: &mut egui::Ui, row: &FleetRowView, selected: bool) -> egui::Response {
    let bg = if selected {
        egui::Color32::from_rgb(0x3a, 0x6a, 0x9c)
    } else {
        egui::Color32::TRANSPARENT
    };
    let mut frame = egui::Frame::none().fill(bg);
    frame.inner_margin = egui::Margin::symmetric(4.0, 1.0);
    let resp = frame.show(ui, |ui| draw_row_inner(ui, row)).response;
    resp.interact(egui::Sense::click())
}

fn draw_row_inner(ui: &mut egui::Ui, row: &FleetRowView) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(&row.status_label)
                .color(status_color(row.status))
                .monospace(),
        );
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
    });
}
