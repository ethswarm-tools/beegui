//! S5 Warmup screen. Renders the warmup checklist from
//! [`bee_cockpit_core::views::warmup::view_for`].

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use bee_cockpit_core::views::warmup::{DEPTH_STABILITY_WINDOW, StepState, WarmupStep, view_for};
use bee_cockpit_core::watch::BeeWatch;

/// Renderer-local state the pure view depends on but can't compute
/// itself: when we first observed `is_warming_up=true` (for the
/// elapsed counter) and the recent depth observations (for the
/// stability check).
#[derive(Debug, Default)]
pub struct WarmupState {
    first_observed: Option<Instant>,
    last_warming_up: bool,
    depth_history: VecDeque<u8>,
}

impl WarmupState {
    fn observe(&mut self, is_warming_up: bool, depth: Option<u8>) -> (Option<Duration>, bool) {
        if is_warming_up && self.first_observed.is_none() {
            self.first_observed = Some(Instant::now());
        }
        self.last_warming_up = is_warming_up;
        if let Some(d) = depth {
            self.depth_history.push_back(d);
            while self.depth_history.len() > DEPTH_STABILITY_WINDOW {
                self.depth_history.pop_front();
            }
        }
        let elapsed = self.first_observed.map(|t| t.elapsed());
        let depth_stable = self.depth_history.len() >= DEPTH_STABILITY_WINDOW
            && self.depth_history.iter().all(|d| Some(*d) == depth);
        (elapsed, depth_stable)
    }
}

pub fn draw(ui: &mut egui::Ui, watch: &BeeWatch, state: &mut WarmupState) {
    let health = watch.health().borrow().clone();
    let stamps = watch.stamps().borrow().clone();
    let topology = watch.topology().borrow().clone();
    let depth = topology.topology.as_ref().map(|t| t.depth);
    let is_warming_up = health
        .status
        .as_ref()
        .map(|s| s.is_warming_up)
        .unwrap_or(false);
    let (elapsed, depth_stable) = state.observe(is_warming_up, depth);
    let view = view_for(&health, &stamps, &topology, elapsed, depth_stable);

    ui.horizontal(|ui| {
        let badge = if view.is_warming_up {
            egui::RichText::new("warming up").color(egui::Color32::from_rgb(0xe0, 0xb0, 0x30))
        } else {
            egui::RichText::new("complete").color(egui::Color32::from_rgb(0x4a, 0xc0, 0x4a))
        };
        ui.label(badge.strong());
        if let Some(e) = view.elapsed {
            ui.label(
                egui::RichText::new(format!("elapsed {}", format_duration(e)))
                    .monospace()
                    .weak(),
            );
        }
    });
    ui.add_space(8.0);

    for step in &view.steps {
        draw_step(ui, step);
        ui.add_space(6.0);
    }
}

fn draw_step(ui: &mut egui::Ui, step: &WarmupStep) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(state_glyph(step.state)).color(state_color(step.state)));
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(step.label).strong());
            ui.label(egui::RichText::new(&step.detail).monospace().weak().small());
            if let StepState::InProgress(pct) = step.state {
                ui.add(egui::ProgressBar::new(pct as f32 / 100.0).desired_width(280.0));
            }
        });
    });
}

fn state_glyph(s: StepState) -> &'static str {
    match s {
        StepState::Done => "✓",
        StepState::InProgress(_) => "▒",
        StepState::Pending => "░",
        StepState::Unknown => "·",
    }
}

fn state_color(s: StepState) -> egui::Color32 {
    match s {
        StepState::Done => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        StepState::InProgress(_) => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        StepState::Pending => egui::Color32::GRAY,
        StepState::Unknown => egui::Color32::DARK_GRAY,
    }
}

fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{}h {}m", s / 3600, (s % 3600) / 60)
    }
}
