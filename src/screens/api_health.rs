//! S8 API Health screen. Renders chain state + pending tx + call stats
//! placeholder. Call-stats require log capture, which isn't wired up
//! in beegui's v0.2 — that lands with the log pane.

use bee_cockpit_core::log_capture::LogEntry;
use bee_cockpit_core::views::api_health::{ApiHealthView, PendingTxRow, view_for};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch, url: &str) {
    let health = watch.health().borrow().clone();
    let transactions = watch.transactions().borrow().clone();
    let empty: Vec<LogEntry> = Vec::new();
    let view = view_for(url, &empty, &health, &transactions);

    egui::ScrollArea::vertical().show(ui, |ui| {
        draw_endpoint(ui, &view);
        ui.add_space(12.0);
        draw_chain(ui, &view);
        ui.add_space(12.0);
        draw_pending(ui, &view.pending);
        ui.add_space(12.0);
        draw_stats(ui, &view);
    });
}

fn draw_endpoint(ui: &mut egui::Ui, view: &ApiHealthView) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("endpoint").weak());
        ui.label(egui::RichText::new(&view.bee_endpoint).monospace());
    });
}

fn draw_chain(ui: &mut egui::Ui, view: &ApiHealthView) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new("Chain state").strong());
        let chain = &view.chain;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(
                    chain
                        .block
                        .map(|b| format!("block {b}"))
                        .unwrap_or_else(|| "block —".into()),
                )
                .monospace(),
            );
            ui.label(
                egui::RichText::new(
                    chain
                        .chain_tip
                        .map(|t| format!("tip {t}"))
                        .unwrap_or_else(|| "tip —".into()),
                )
                .monospace(),
            );
            ui.label(
                egui::RichText::new(
                    chain
                        .delta
                        .map(|d| format!("Δ {d:+}"))
                        .unwrap_or_else(|| "Δ —".into()),
                )
                .monospace()
                .color(delta_color(chain.delta)),
            );
        });
        if let Some(price) = &chain.current_price {
            ui.label(egui::RichText::new(format!("price {price}")).monospace());
        }
        if let Some(total) = &chain.total_amount {
            ui.label(egui::RichText::new(format!("total {total}")).monospace());
        }
    });
}

fn draw_pending(ui: &mut egui::Ui, rows: &[PendingTxRow]) {
    ui.label(egui::RichText::new(format!("Pending transactions ({})", rows.len())).strong());
    if rows.is_empty() {
        ui.label(egui::RichText::new("(none)").italics().weak());
        return;
    }
    egui::Grid::new("pending-tx")
        .num_columns(5)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("nonce").strong());
            ui.label(egui::RichText::new("hash").strong());
            ui.label(egui::RichText::new("to").strong());
            ui.label(egui::RichText::new("created").strong());
            ui.label(egui::RichText::new("description").strong());
            ui.end_row();
            for row in rows {
                ui.label(egui::RichText::new(row.nonce.to_string()).monospace());
                ui.label(egui::RichText::new(&row.hash_short).monospace());
                ui.label(egui::RichText::new(&row.to_short).monospace());
                ui.label(egui::RichText::new(&row.created).weak());
                ui.label(&row.description);
                ui.end_row();
            }
        });
}

fn draw_stats(ui: &mut egui::Ui, view: &ApiHealthView) {
    let cs = &view.call_stats;
    ui.label(egui::RichText::new("HTTP call stats").strong());
    if cs.sample_size == 0 {
        ui.label(
            egui::RichText::new("call-stats need the log pane (not yet wired in beegui v0.2)")
                .italics()
                .weak(),
        );
        return;
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("samples {}", cs.sample_size)).monospace());
        ui.label(
            egui::RichText::new(format!(
                "p50 {}",
                cs.p50_ms.map(|v| format!("{v}ms")).unwrap_or("—".into())
            ))
            .monospace(),
        );
        ui.label(
            egui::RichText::new(format!(
                "p99 {}",
                cs.p99_ms.map(|v| format!("{v}ms")).unwrap_or("—".into())
            ))
            .monospace(),
        );
        ui.label(egui::RichText::new(format!("error {:.1}%", cs.error_rate_pct)).monospace());
    });
}

fn delta_color(d: Option<i64>) -> egui::Color32 {
    match d {
        Some(0) => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        Some(n) if n.unsigned_abs() < 50 => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        Some(_) => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        None => egui::Color32::GRAY,
    }
}
