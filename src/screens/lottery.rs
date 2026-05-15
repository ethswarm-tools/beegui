//! S4 Lottery / redistribution screen.

use bee_cockpit_core::views::lottery::{
    AnchorRow, Phase, PhaseSegment, PhaseState, RoundCard, StakeCard, StakeStatus, view_for,
};
use bee_cockpit_core::watch::BeeWatch;

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch) {
    let health = watch.health().borrow().clone();
    let lottery = watch.lottery().borrow().clone();
    let view = view_for(&health, &lottery);

    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(round) = &view.round {
            draw_round(ui, round);
        } else {
            ui.label(
                egui::RichText::new("waiting for redistribution state…")
                    .italics()
                    .weak(),
            );
        }
        ui.add_space(12.0);
        draw_anchors(ui, &view.anchors);
        ui.add_space(12.0);
        draw_stake(ui, &view.stake);
    });
}

fn draw_round(ui: &mut egui::Ui, round: &RoundCard) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("round {}", round.round)).strong());
            ui.label(
                egui::RichText::new(format!("block {} (+{})", round.block, round.block_of_round))
                    .monospace()
                    .weak(),
            );
            ui.label(
                egui::RichText::new(round.phase_label)
                    .color(phase_color(round.phase))
                    .strong(),
            );
        });
        ui.add_space(4.0);
        draw_phase_ribbon(ui, &round.segments);
    });
}

fn draw_phase_ribbon(ui: &mut egui::Ui, segments: &[PhaseSegment]) {
    ui.horizontal(|ui| {
        for seg in segments {
            let mut color = phase_color(seg.phase);
            if seg.state != PhaseState::Active {
                color = color.linear_multiply(0.4);
            }
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(60.0, 14.0),
                egui::Sense::focusable_noninteractive(),
            );
            ui.painter().rect_filled(rect, 2.0, color);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                seg.phase.label(),
                egui::FontId::monospace(10.0),
                egui::Color32::BLACK,
            );
        }
    });
}

fn draw_anchors(ui: &mut egui::Ui, rows: &[AnchorRow]) {
    ui.label(egui::RichText::new("Anchors").strong());
    if rows.is_empty() {
        ui.label(egui::RichText::new("(no history)").italics().weak());
        return;
    }
    egui::Grid::new("anchors")
        .num_columns(4)
        .spacing([16.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for row in rows {
                ui.label(row.label);
                ui.label(egui::RichText::new(format!("round {}", row.round)).monospace());
                ui.label(
                    egui::RichText::new(
                        row.delta
                            .map(|d| format!("Δ {d}"))
                            .unwrap_or_else(|| "—".into()),
                    )
                    .monospace(),
                );
                ui.label(egui::RichText::new(&row.when).weak());
                ui.end_row();
            }
        });
}

fn draw_stake(ui: &mut egui::Ui, card: &StakeCard) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(card.status.label()).color(stake_color(card.status)));
            ui.label(egui::RichText::new("stake").strong());
        });
        ui.label(egui::RichText::new(format!("staked {}", card.staked)).monospace());
        ui.label(egui::RichText::new(format!("min gas {}", card.minimum_gas)).monospace());
        ui.label(egui::RichText::new(format!("reward {}", card.reward)).monospace());
        ui.label(egui::RichText::new(format!("fees {}", card.fees)).monospace());
        if let Some(ls) = &card.last_sample {
            ui.label(
                egui::RichText::new(format!("last sample {ls}"))
                    .monospace()
                    .weak(),
            );
        }
        if let Some(why) = &card.why {
            ui.label(egui::RichText::new(why).italics().weak().small());
        }
    });
}

fn phase_color(p: Phase) -> egui::Color32 {
    match p {
        Phase::Commit => egui::Color32::from_rgb(0x4a, 0x9c, 0xe0),
        Phase::Reveal => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        Phase::Claim => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        Phase::Sample => egui::Color32::from_rgb(0x9c, 0x70, 0xd0),
        Phase::Unknown => egui::Color32::GRAY,
    }
}

fn stake_color(s: StakeStatus) -> egui::Color32 {
    match s {
        StakeStatus::Healthy => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        StakeStatus::InsufficientGas | StakeStatus::Unhealthy => {
            egui::Color32::from_rgb(0xe0, 0xb0, 0x30)
        }
        StakeStatus::Unstaked | StakeStatus::Frozen => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        StakeStatus::Unknown => egui::Color32::GRAY,
    }
}
