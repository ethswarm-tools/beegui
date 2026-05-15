//! S3 Swap screen. Renders [`bee_cockpit_core::views::swap::view_for_no_market`].

use bee_cockpit_core::views::swap::{
    ChequebookCard, CheckRow, SettlementRow, SwapStatus, SwapView, view_for_no_market,
};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch) {
    let snap = watch.swap().borrow().clone();
    let view = view_for_no_market(&snap);

    egui::ScrollArea::vertical().show(ui, |ui| {
        draw_card(ui, &view.card);
        if let Some(addr) = &view.chequebook_address {
            ui.label(
                egui::RichText::new(format!("chequebook: {addr}"))
                    .monospace()
                    .weak()
                    .small(),
            );
        }
        ui.add_space(12.0);
        draw_totals(ui, &view);
        ui.add_space(12.0);
        draw_cheques(ui, &view.cheques);
        ui.add_space(12.0);
        draw_settlements(ui, &view.settlements);
    });
}

fn draw_card(ui: &mut egui::Ui, card: &ChequebookCard) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(card.status.label()).color(status_color(card.status)));
            ui.label(egui::RichText::new("chequebook").strong());
        });
        ui.label(egui::RichText::new(format!("total {}", card.total)).monospace());
        ui.label(egui::RichText::new(format!("available {}", card.available)).monospace());
        ui.add(egui::ProgressBar::new(card.available_pct as f32 / 100.0).desired_width(320.0));
        if let Some(why) = &card.why {
            ui.label(egui::RichText::new(why).italics().weak().small());
        }
    });
}

fn draw_totals(ui: &mut egui::Ui, view: &SwapView) {
    ui.horizontal(|ui| {
        if let Some(t) = &view.time_total_received {
            ui.label(egui::RichText::new(format!("time-total received: {t}")).monospace());
        }
        if let Some(t) = &view.time_total_sent {
            ui.label(egui::RichText::new(format!("sent: {t}")).monospace());
        }
    });
}

fn draw_cheques(ui: &mut egui::Ui, rows: &[CheckRow]) {
    ui.label(egui::RichText::new("Last received cheques").strong());
    if rows.is_empty() {
        ui.label(egui::RichText::new("(none)").italics().weak());
        return;
    }
    egui::Grid::new("cheques")
        .num_columns(2)
        .spacing([16.0, 2.0])
        .show(ui, |ui| {
            for row in rows {
                ui.label(egui::RichText::new(&row.peer_short).monospace());
                ui.label(
                    egui::RichText::new(&row.payout)
                        .monospace()
                        .color(if row.never {
                            egui::Color32::GRAY
                        } else {
                            egui::Color32::WHITE
                        }),
                );
                ui.end_row();
            }
        });
}

fn draw_settlements(ui: &mut egui::Ui, rows: &[SettlementRow]) {
    ui.label(egui::RichText::new("Per-peer settlements").strong());
    if rows.is_empty() {
        ui.label(egui::RichText::new("(none)").italics().weak());
        return;
    }
    egui::Grid::new("settlements")
        .num_columns(4)
        .spacing([16.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("peer").strong());
            ui.label(egui::RichText::new("received").strong());
            ui.label(egui::RichText::new("sent").strong());
            ui.label(egui::RichText::new("net").strong());
            ui.end_row();
            for row in rows {
                ui.label(egui::RichText::new(&row.peer_short).monospace());
                ui.label(egui::RichText::new(&row.received).monospace());
                ui.label(egui::RichText::new(&row.sent).monospace());
                let net = egui::RichText::new(&row.net).monospace();
                ui.label(if row.net_flagged {
                    net.color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a))
                } else {
                    net
                });
                ui.end_row();
            }
        });
}

fn status_color(s: SwapStatus) -> egui::Color32 {
    match s {
        SwapStatus::Healthy => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        SwapStatus::Tight => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        SwapStatus::Empty => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        SwapStatus::Unknown => egui::Color32::GRAY,
    }
}
