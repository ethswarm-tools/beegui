//! S13 Feed timeline screen.

pub fn draw(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.heading("Feed Timeline");
        ui.label(
            egui::RichText::new("walks a feed and tabulates index, age, payload.")
                .italics()
                .weak(),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("(owner+topic input + walker land in beegui v0.3)")
                .weak()
                .small(),
        );
    });
}
