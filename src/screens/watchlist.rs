//! S12 Watchlist screen. Operator adds references; beegui re-checks
//! them in the background via [`bee_cockpit_core::durability::check`]
//! and renders the rolling results.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::SystemTime;

use bee::swarm::Reference;
use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::durability::{DurabilityResult, check};
use bee_cockpit_core::views::manifest::parse_hex_32;
use bee_cockpit_core::views::watchlist::{WatchlistRow, view_for};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

const HISTORY_CAP: usize = 50;

pub struct WatchlistState {
    input: String,
    error: Option<String>,
    refs: Vec<Reference>,
    history: VecDeque<DurabilityResult>,
    inflight: usize,
    selected: usize,
    incoming: mpsc::UnboundedReceiver<DurabilityResult>,
    incoming_tx: mpsc::UnboundedSender<DurabilityResult>,
}

impl Default for WatchlistState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            input: String::new(),
            error: None,
            refs: Vec::new(),
            history: VecDeque::new(),
            inflight: 0,
            selected: 0,
            incoming: rx,
            incoming_tx: tx,
        }
    }
}

impl WatchlistState {
    pub fn add_external(&mut self, reference: String, api: &Arc<ApiClient>, rt: &Handle) {
        self.input = reference;
        add_ref(self, api, rt);
    }
}

pub fn draw(ui: &mut egui::Ui, state: &mut WatchlistState, api: Arc<ApiClient>, rt: &Handle) {
    drain(state);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("reference").weak());
        let r = ui.add(
            egui::TextEdit::singleline(&mut state.input)
                .desired_width(420.0)
                .hint_text("paste a 32-byte hex reference"),
        );
        let add = ui.button("Add + check");
        if (add.clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
            && !state.input.trim().is_empty()
        {
            add_ref(state, &api, rt);
        }
        if !state.refs.is_empty() && ui.button("Re-check all").clicked() {
            recheck_all(state, &api, rt);
        }
    });
    if let Some(err) = &state.error {
        ui.label(egui::RichText::new(err).color(egui::Color32::RED).small());
    }
    ui.add_space(8.0);

    let view = view_for(&state.history, SystemTime::now());

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("healthy {}", view.healthy_count))
                .color(egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)),
        );
        ui.label(
            egui::RichText::new(format!("unhealthy {}", view.unhealthy_count))
                .color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)),
        );
        ui.label(egui::RichText::new(format!("watching {}", state.refs.len())).weak());
        if state.inflight > 0 {
            ui.label(
                egui::RichText::new(format!("({} in-flight)", state.inflight))
                    .italics()
                    .weak(),
            );
        }
    });
    ui.add_space(8.0);

    if view.rows.is_empty() && state.refs.is_empty() {
        ui.label(
            egui::RichText::new("no references being watched — add one above.")
                .italics()
                .weak(),
        );
        return;
    }

    let n = view.rows.len();
    if !ui.ctx().memory(|m| m.focused().is_some()) {
        ui.input(|i| {
            if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K) {
                state.selected = state.selected.saturating_sub(1);
            }
            if (i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J))
                && state.selected + 1 < n
            {
                state.selected += 1;
            }
            if i.key_pressed(egui::Key::PageUp) {
                state.selected = state.selected.saturating_sub(10);
            }
            if i.key_pressed(egui::Key::PageDown) {
                state.selected = (state.selected + 10).min(n.saturating_sub(1));
            }
            if i.key_pressed(egui::Key::Home) {
                state.selected = 0;
            }
            if i.key_pressed(egui::Key::End) && n > 0 {
                state.selected = n - 1;
            }
        });
    }
    if state.selected >= n.max(1) {
        state.selected = n.saturating_sub(1);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, row) in view.rows.iter().enumerate() {
            let resp = draw_row(ui, row, i == state.selected);
            if resp.clicked() {
                state.selected = i;
            }
        }
    });
}

fn draw_row(ui: &mut egui::Ui, row: &WatchlistRow, selected: bool) -> egui::Response {
    let bg = if selected {
        egui::Color32::from_rgb(0x3a, 0x6a, 0x9c)
    } else {
        egui::Color32::TRANSPARENT
    };
    let mut frame = egui::Frame::none().fill(bg);
    frame.inner_margin = egui::Margin::symmetric(4.0, 1.0);
    let color = if row.healthy {
        egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)
    } else {
        egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)
    };
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&row.status_label).color(color));
                ui.label(egui::RichText::new(&row.reference_hex[..16]).monospace().weak());
                ui.label(egui::RichText::new(&row.detail).monospace());
                ui.label(
                    egui::RichText::new(format_age(row.age_seconds))
                        .monospace()
                        .weak(),
                );
            });
        })
        .response;
    resp.interact(egui::Sense::click())
}

fn add_ref(state: &mut WatchlistState, api: &Arc<ApiClient>, rt: &Handle) {
    state.error = None;
    let trimmed = state.input.trim().trim_start_matches("0x").to_string();
    let bytes = match parse_hex_32(&trimmed) {
        Ok(b) => b,
        Err(e) => {
            state.error = Some(e);
            return;
        }
    };
    let reference = match Reference::new(&bytes) {
        Ok(r) => r,
        Err(e) => {
            state.error = Some(format!("reference: {e}"));
            return;
        }
    };
    if !state.refs.contains(&reference) {
        state.refs.push(reference.clone());
    }
    state.input.clear();
    spawn_check(state, reference, api, rt);
}

fn recheck_all(state: &mut WatchlistState, api: &Arc<ApiClient>, rt: &Handle) {
    let refs = state.refs.clone();
    for r in refs {
        spawn_check(state, r, api, rt);
    }
}

fn spawn_check(
    state: &mut WatchlistState,
    reference: Reference,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    state.inflight += 1;
    let api = api.clone();
    let tx = state.incoming_tx.clone();
    rt.spawn(async move {
        let r = check(api, reference).await;
        let _ = tx.send(r);
    });
}

fn drain(state: &mut WatchlistState) {
    while let Ok(result) = state.incoming.try_recv() {
        state.inflight = state.inflight.saturating_sub(1);
        if state.history.len() >= HISTORY_CAP {
            state.history.pop_front();
        }
        state.history.push_back(result);
    }
}

fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}
