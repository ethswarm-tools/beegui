//! S15 Fleet screen. The fleet roll-up requires the fleet poller
//! (one BeeWatch per node + an aggregator); single-node operation
//! is sufficient for beegui v0.2.

pub fn draw(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.heading("Fleet");
        ui.label(
            egui::RichText::new("aggregate health across multiple Bee nodes.")
                .italics()
                .weak(),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("(multi-node poller lands in beegui v0.3)")
                .weak()
                .small(),
        );
    });
}
