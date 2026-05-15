//! S11 Manifest screen. Manifest walking requires a root reference
//! input + an async walker run; this lands with the command bar in
//! Phase 3. For v0.2 the screen documents the upcoming feature.

pub fn draw(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.heading("Manifest");
        ui.label(
            egui::RichText::new("interactive walker — type a root reference to load.")
                .italics()
                .weak(),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("(input box lands with the command bar in beegui v0.3)")
                .weak()
                .small(),
        );
    });
}
