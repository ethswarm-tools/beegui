//! S7 Network screen. Renders [`bee_cockpit_core::views::network::view_for`].

use bee_cockpit_core::views::network::{
    AvailabilityStatus, NetworkView, ReachabilityStatus, UnderlayKind, UnderlayRow, view_for,
};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch) {
    let network = watch.network().borrow().clone();
    let topology = watch.topology().borrow().clone();
    let view = view_for(&network, &topology);

    egui::ScrollArea::vertical().show(ui, |ui| {
        draw_identity(ui, &view);
        ui.add_space(12.0);
        draw_reachability(ui, &view);
        ui.add_space(12.0);
        draw_underlays(ui, &view.underlays);
    });
}

fn draw_identity(ui: &mut egui::Ui, view: &NetworkView) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new("Identity").strong());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("overlay").weak());
            ui.label(egui::RichText::new(&view.overlay_full).monospace());
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("ethereum").weak());
            ui.label(egui::RichText::new(&view.ethereum_full).monospace());
        });
    });
}

fn draw_reachability(ui: &mut egui::Ui, view: &NetworkView) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new("Reachability").strong());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("status").weak());
            ui.label(
                egui::RichText::new(view.reachability.label())
                    .color(reach_color(&view.reachability))
                    .strong(),
            );
            ui.label(egui::RichText::new("availability").weak());
            ui.label(
                egui::RichText::new(view.network_availability.label())
                    .color(avail_color(&view.network_availability)),
            );
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("inbound  {}", view.inbound)).monospace());
            ui.label(egui::RichText::new(format!("outbound {}", view.outbound)).monospace());
        });
    });
}

fn draw_underlays(ui: &mut egui::Ui, rows: &[UnderlayRow]) {
    ui.label(egui::RichText::new(format!("Underlays ({})", rows.len())).strong());
    if rows.is_empty() {
        ui.label(egui::RichText::new("(none)").italics().weak());
        return;
    }
    egui::Grid::new("underlays")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for row in rows {
                ui.label(egui::RichText::new(kind_label(row.kind)).color(kind_color(row.kind)));
                ui.label(egui::RichText::new(&row.multiaddr).monospace());
                ui.end_row();
            }
        });
}

fn reach_color(r: &ReachabilityStatus) -> egui::Color32 {
    match r {
        ReachabilityStatus::Public => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        ReachabilityStatus::Private => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        ReachabilityStatus::NotLoaded => egui::Color32::GRAY,
        ReachabilityStatus::Other(_) => egui::Color32::DARK_GRAY,
    }
}

fn avail_color(a: &AvailabilityStatus) -> egui::Color32 {
    match a {
        AvailabilityStatus::Available => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        AvailabilityStatus::Unavailable => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        AvailabilityStatus::NotLoaded => egui::Color32::GRAY,
        AvailabilityStatus::Other(_) => egui::Color32::DARK_GRAY,
    }
}

fn kind_label(k: UnderlayKind) -> &'static str {
    match k {
        UnderlayKind::Public => "public",
        UnderlayKind::Private => "private",
        UnderlayKind::Unknown => "unknown",
    }
}

fn kind_color(k: UnderlayKind) -> egui::Color32 {
    match k {
        UnderlayKind::Public => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        UnderlayKind::Private => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        UnderlayKind::Unknown => egui::Color32::GRAY,
    }
}
