//! S2 Stamps screen. Table + click-or-Enter drill into bucket
//! distribution via [`bee_cockpit_core::views::stamps::compute_drill_view`].

use std::sync::Arc;

use bee::postage::{PostageBatch, PostageBatchBuckets};
use bee::swarm::BatchId;
use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::views::stamps::{
    FILL_BIN_LABELS, StampDrillView, StampRow, StampStatus, compute_drill_view, rows_for,
};
use bee_cockpit_core::watch::BeeWatch;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
enum DrillState {
    Idle,
    Loading {
        batch_id: BatchId,
    },
    Loaded {
        batch_id: BatchId,
        view: Box<StampDrillView>,
    },
    Failed {
        batch_id: BatchId,
        err: String,
    },
}

pub struct StampsScreenState {
    selected: usize,
    drill: DrillState,
    incoming: mpsc::UnboundedReceiver<(BatchId, Result<PostageBatchBuckets, String>)>,
    incoming_tx: mpsc::UnboundedSender<(BatchId, Result<PostageBatchBuckets, String>)>,
}

impl Default for StampsScreenState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            selected: 0,
            drill: DrillState::Idle,
            incoming: rx,
            incoming_tx: tx,
        }
    }
}

impl StampsScreenState {
    fn drill_open(&self) -> bool {
        !matches!(self.drill, DrillState::Idle)
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    watch: &BeeWatch,
    state: &mut StampsScreenState,
    api: Arc<ApiClient>,
    rt: &Handle,
) {
    drain(state, watch);
    let snap = watch.stamps().borrow().clone();
    let rows = rows_for(&snap);
    if state.selected >= rows.len().max(1) {
        state.selected = rows.len().saturating_sub(1);
    }

    handle_keys(ui, state, &snap, &rows, &api, rt);

    if rows.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            if snap.last_update.is_none() {
                ui.label(egui::RichText::new("loading…").italics().weak());
            } else if let Some(err) = &snap.last_error {
                ui.label(egui::RichText::new(format!("error: {err}")).color(egui::Color32::RED));
            } else {
                ui.label(egui::RichText::new("no postage batches").italics().weak());
            }
        });
        return;
    }

    if state.drill_open() {
        ui.columns(2, |cols| {
            draw_table(&mut cols[0], &rows, state, &snap, &api, rt);
            draw_drill(&mut cols[1], state);
        });
    } else {
        draw_table(ui, &rows, state, &snap, &api, rt);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Enter / click to drill into bucket histogram")
                .italics()
                .weak()
                .small(),
        );
    }
}

fn handle_keys(
    ui: &mut egui::Ui,
    state: &mut StampsScreenState,
    snap: &bee_cockpit_core::watch::StampsSnapshot,
    rows: &[StampRow],
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    if ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }
    let n = rows.len();
    let mut up = false;
    let mut down = false;
    let mut enter = false;
    let mut esc = false;
    ui.input(|i| {
        if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K) {
            up = true;
        }
        if i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J) {
            down = true;
        }
        if i.key_pressed(egui::Key::Enter) {
            enter = true;
        }
        if i.key_pressed(egui::Key::Escape) {
            esc = true;
        }
    });
    if esc && !matches!(state.drill, DrillState::Idle) {
        state.drill = DrillState::Idle;
        return;
    }
    if up {
        state.selected = state.selected.saturating_sub(1);
    }
    if down && n > 0 && state.selected + 1 < n {
        state.selected += 1;
    }
    if enter && n > 0 {
        start_drill(state, snap, api, rt);
    }
}

fn draw_table(
    ui: &mut egui::Ui,
    rows: &[StampRow],
    state: &mut StampsScreenState,
    snap: &bee_cockpit_core::watch::StampsSnapshot,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    egui::ScrollArea::vertical()
        .id_salt("stamps")
        .show(ui, |ui| {
            for (i, row) in rows.iter().enumerate() {
                let resp = draw_row(ui, row, i == state.selected);
                if resp.clicked() {
                    state.selected = i;
                    start_drill(state, snap, api, rt);
                }
            }
        });
}

fn draw_row(ui: &mut egui::Ui, row: &StampRow, selected: bool) -> egui::Response {
    let bg = if selected {
        egui::Color32::from_rgb(0x3a, 0x6a, 0x9c)
    } else {
        egui::Color32::TRANSPARENT
    };
    let mut frame = egui::Frame::none().fill(bg);
    frame.inner_margin = egui::Margin::symmetric(4.0, 1.0);
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(row.status.label())
                        .color(status_color(row.status))
                        .monospace(),
                );
                ui.label(egui::RichText::new(&row.label).monospace());
                ui.label(egui::RichText::new(&row.batch_id_short).monospace().weak());
                ui.label(egui::RichText::new(&row.volume).monospace());
                ui.label(
                    egui::RichText::new(format!(
                        "{:>3}% ({})",
                        row.worst_bucket_pct, row.worst_bucket_raw
                    ))
                    .monospace(),
                );
                ui.label(egui::RichText::new(&row.ttl).monospace());
                ui.label(if row.immutable { "immutable" } else { "mutable" });
            });
        })
        .response;
    resp.interact(egui::Sense::click())
}

fn draw_drill(ui: &mut egui::Ui, state: &StampsScreenState) {
    egui::Frame::group(ui.style()).show(ui, |ui| match &state.drill {
        DrillState::Idle => {}
        DrillState::Loading { batch_id } => {
            ui.label(egui::RichText::new("Loading buckets…").strong());
            ui.label(egui::RichText::new(&batch_id.to_hex()[..16]).monospace().weak());
            ui.label(egui::RichText::new("(Esc to close)").italics().weak().small());
        }
        DrillState::Failed { batch_id, err } => {
            ui.label(
                egui::RichText::new("Bucket fetch failed")
                    .strong()
                    .color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)),
            );
            ui.label(egui::RichText::new(&batch_id.to_hex()[..16]).monospace().weak());
            ui.label(egui::RichText::new(err).color(egui::Color32::RED).small());
        }
        DrillState::Loaded { view, .. } => draw_drill_view(ui, view),
    });
}

fn draw_drill_view(ui: &mut egui::Ui, view: &StampDrillView) {
    ui.label(egui::RichText::new("Bucket histogram").strong());
    ui.label(
        egui::RichText::new(format!(
            "depth {} · bucket-depth {} · total chunks {}",
            view.depth, view.bucket_depth, view.total_chunks
        ))
        .monospace()
        .small()
        .weak(),
    );
    ui.label(
        egui::RichText::new(format!(
            "worst bucket {}% (of {} capacity)",
            view.worst_pct, view.upper_bound
        ))
        .monospace(),
    );
    ui.separator();
    egui::Grid::new("hist")
        .num_columns(3)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("range").strong());
            ui.label(egui::RichText::new("buckets").strong());
            ui.label(egui::RichText::new("bar").strong());
            ui.end_row();
            let max = view.fill_distribution.iter().copied().max().unwrap_or(1).max(1);
            for (idx, &count) in view.fill_distribution.iter().enumerate() {
                ui.label(egui::RichText::new(FILL_BIN_LABELS[idx]).monospace());
                ui.label(egui::RichText::new(count.to_string()).monospace());
                let pct = (count as f32) / (max as f32);
                ui.add(egui::ProgressBar::new(pct).desired_width(140.0));
                ui.end_row();
            }
        });
    if !view.worst_buckets.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Top buckets by collisions").strong());
        egui::Grid::new("worst")
            .num_columns(3)
            .spacing([12.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("bucket").strong());
                ui.label(egui::RichText::new("collisions").strong());
                ui.label(egui::RichText::new("fill").strong());
                ui.end_row();
                for w in &view.worst_buckets {
                    ui.label(egui::RichText::new(w.bucket_id.to_string()).monospace());
                    ui.label(egui::RichText::new(w.collisions.to_string()).monospace());
                    ui.label(egui::RichText::new(format!("{}%", w.pct)).monospace());
                    ui.end_row();
                }
            });
    }
    if let Some(econ) = &view.economics {
        ui.separator();
        ui.label(egui::RichText::new("Economics").strong());
        ui.label(egui::RichText::new(format!("paid {}", econ.bzz_paid)).monospace());
        ui.label(egui::RichText::new(format!("volume {}", econ.volume_humanised)).monospace());
        ui.label(egui::RichText::new(format!("BZZ/GiB {}", econ.bzz_per_gib)).monospace());
    }
    ui.separator();
    ui.label(egui::RichText::new("Esc · close drill").italics().weak().small());
}

fn start_drill(
    state: &mut StampsScreenState,
    snap: &bee_cockpit_core::watch::StampsSnapshot,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    let Some(batch) = snap.batches.get(state.selected) else {
        return;
    };
    let batch_id = batch.batch_id;
    if let DrillState::Loading { batch_id: pending } = &state.drill {
        if *pending == batch_id {
            return;
        }
    }
    state.drill = DrillState::Loading { batch_id };
    let tx = state.incoming_tx.clone();
    let api = api.clone();
    rt.spawn(async move {
        let res = api
            .bee()
            .postage()
            .get_postage_batch_buckets(&batch_id)
            .await
            .map_err(|e| e.to_string());
        let _ = tx.send((batch_id, res));
    });
}

fn drain(state: &mut StampsScreenState, watch: &BeeWatch) {
    let snap = watch.stamps().borrow().clone();
    while let Ok((batch_id, result)) = state.incoming.try_recv() {
        let pending = match &state.drill {
            DrillState::Loading { batch_id: p } => *p,
            _ => continue,
        };
        if pending != batch_id {
            continue;
        }
        state.drill = match result {
            Err(e) => DrillState::Failed { batch_id, err: e },
            Ok(buckets) => {
                let batch: Option<&PostageBatch> =
                    snap.batches.iter().find(|b| b.batch_id == batch_id);
                let view = compute_drill_view(&buckets, batch);
                DrillState::Loaded {
                    batch_id,
                    view: Box::new(view),
                }
            }
        };
    }
}

fn status_color(s: StampStatus) -> egui::Color32 {
    match s {
        StampStatus::Healthy => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        StampStatus::Skewed => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
        StampStatus::Critical | StampStatus::Expired => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        StampStatus::Pending => egui::Color32::GRAY,
    }
}
