//! S14 Pubsub screen.

pub fn draw(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.heading("Pubsub");
        ui.label(
            egui::RichText::new("subscribe to PSS / GSOC and tail the message stream.")
                .italics()
                .weak(),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("(subscription worker lands in beegui v0.3)")
                .weak()
                .small(),
        );
    });
}
