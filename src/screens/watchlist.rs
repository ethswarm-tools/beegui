//! S12 Watchlist screen. Durability rolling checks require the
//! durability worker (a separate poller) — wired up in Phase 3.

pub fn draw(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.heading("Watchlist");
        ui.label(
            egui::RichText::new("references currently being durability-checked.")
                .italics()
                .weak(),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("(durability worker lands in beegui v0.3)")
                .weak()
                .small(),
        );
    });
}
