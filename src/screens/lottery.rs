//! S4 Lottery / redistribution screen. `r` triggers an rchash
//! benchmark against the depth-derived sample (parity with
//! bee-tui's S4).

use std::sync::Arc;

use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::views::lottery::{
    AnchorRow, Phase, PhaseSegment, PhaseState, RoundCard, StakeCard, StakeStatus, bench_depth,
    view_for,
};
use bee_cockpit_core::watch::BeeWatch;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

const BENCH_ANCHOR_LO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const BENCH_ANCHOR_HI: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

#[derive(Debug, Clone)]
enum BenchState {
    Idle,
    Running,
    Done { duration_seconds: f64, hash: String },
    Failed { error: String },
}

pub struct LotteryScreenState {
    bench: BenchState,
    incoming:
        mpsc::UnboundedReceiver<std::result::Result<bee::debug::RCHashResponse, String>>,
    incoming_tx:
        mpsc::UnboundedSender<std::result::Result<bee::debug::RCHashResponse, String>>,
}

impl Default for LotteryScreenState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            bench: BenchState::Idle,
            incoming: rx,
            incoming_tx: tx,
        }
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    watch: &BeeWatch,
    state: &mut LotteryScreenState,
    api: Arc<ApiClient>,
    rt: &Handle,
) {
    drain(state);

    let health = watch.health().borrow().clone();
    let lottery = watch.lottery().borrow().clone();
    let view = view_for(&health, &lottery);

    if !ui.ctx().memory(|m| m.focused().is_some()) {
        let mut start = false;
        ui.input(|i| {
            if i.key_pressed(egui::Key::R) {
                start = true;
            }
        });
        if start {
            start_bench(state, &health, &api, rt);
        }
    }

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
        ui.add_space(8.0);
        draw_bench(ui, state, &health, &api, rt);
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

fn draw_bench(
    ui: &mut egui::Ui,
    state: &mut LotteryScreenState,
    health: &bee_cockpit_core::watch::HealthSnapshot,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    let depth = bench_depth(health);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("rchash benchmark").strong());
            ui.label(
                egui::RichText::new(format!("depth {depth}"))
                    .monospace()
                    .weak()
                    .small(),
            );
            let running = matches!(state.bench, BenchState::Running);
            let label = if running { "running…" } else { "Run (r)" };
            let btn = ui.add_enabled(!running, egui::Button::new(label));
            if btn.clicked() {
                start_bench(state, health, api, rt);
            }
        });
        match &state.bench {
            BenchState::Idle => {
                ui.label(
                    egui::RichText::new(
                        "Press r (or click Run) to time the redistribution sample lookup.",
                    )
                    .italics()
                    .weak()
                    .small(),
                );
            }
            BenchState::Running => {
                ui.label(
                    egui::RichText::new("running… (seconds to minutes on a busy reserve)")
                        .italics()
                        .weak(),
                );
            }
            BenchState::Done {
                duration_seconds,
                hash,
            } => {
                let safe = *duration_seconds < 95.0;
                let color = if safe {
                    egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)
                } else {
                    egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)
                };
                let verdict = if safe { "OK" } else { "SLOW" };
                ui.label(
                    egui::RichText::new(format!(
                        "{verdict}  ·  {:.2}s  ·  hash {}",
                        duration_seconds,
                        &hash[..16.min(hash.len())]
                    ))
                    .color(color)
                    .monospace(),
                );
                if !safe {
                    ui.label(
                        egui::RichText::new(
                            "lookup is too slow — the reveal phase may time out at 95s",
                        )
                        .italics()
                        .small()
                        .color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)),
                    );
                }
            }
            BenchState::Failed { error } => {
                ui.label(
                    egui::RichText::new(format!("failed: {error}"))
                        .color(egui::Color32::RED)
                        .small(),
                );
            }
        }
    });
}

fn start_bench(
    state: &mut LotteryScreenState,
    health: &bee_cockpit_core::watch::HealthSnapshot,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    if matches!(state.bench, BenchState::Running) {
        return;
    }
    let depth = bench_depth(health);
    state.bench = BenchState::Running;
    let api = api.clone();
    let tx = state.incoming_tx.clone();
    rt.spawn(async move {
        let res = api
            .bee()
            .debug()
            .r_chash(depth, BENCH_ANCHOR_LO, BENCH_ANCHOR_HI)
            .await
            .map_err(|e| e.to_string());
        let _ = tx.send(res);
    });
}

fn drain(state: &mut LotteryScreenState) {
    while let Ok(result) = state.incoming.try_recv() {
        state.bench = match result {
            Ok(r) => BenchState::Done {
                duration_seconds: r.duration_seconds,
                hash: r.hash,
            },
            Err(e) => BenchState::Failed { error: e },
        };
    }
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
