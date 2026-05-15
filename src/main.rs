//! beegui — desktop GUI cockpit for Ethereum Swarm Bee node operators.
//!
//! Sibling of [bee-tui]. Same cockpit logic — health gates, stamp
//! warnings, fleet roll-up, redistribution skip reasons — via the
//! shared [bee-cockpit-core] crate, rendered with [egui] instead of
//! ratatui.
//!
//! 🚧 Scaffold only. The renderer + first screen land in a follow-up
//! session; see `bee-cockpit-core/PLAN.md` for the extraction
//! roadmap.
//!
//! [bee-tui]: https://github.com/ethswarm-tools/bee-tui
//! [bee-cockpit-core]: https://github.com/ethswarm-tools/bee-cockpit-core
//! [egui]: https://github.com/emilk/egui

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "beegui",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}

#[derive(Default)]
struct App {}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("beegui");
            ui.label("Desktop GUI cockpit for Ethereum Swarm Bee — scaffold.");
            ui.label("Rendering of the first screen lands once bee-cockpit-core is extracted.");
        });
    }
}
