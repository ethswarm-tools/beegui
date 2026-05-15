//! S14 Pubsub screen. Subscribes to PSS by topic and tails the
//! message stream into a ring buffer.

use std::collections::VecDeque;
use std::sync::Arc;

use bee::swarm::Topic;
use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::pubsub::{PubsubMessage, spawn_pss_watcher};
use bee_cockpit_core::views::pubsub::{PubsubRowView, view_for};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const MESSAGES_CAP: usize = 200;

pub struct PubsubState {
    topic_input: String,
    filter_input: String,
    error: Option<String>,
    messages: VecDeque<PubsubMessage>,
    sub: Option<Subscription>,
    incoming: mpsc::UnboundedReceiver<PubsubMessage>,
    incoming_tx: mpsc::UnboundedSender<PubsubMessage>,
}

struct Subscription {
    topic_hex: String,
    cancel: CancellationToken,
}

impl Default for PubsubState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            topic_input: String::new(),
            filter_input: String::new(),
            error: None,
            messages: VecDeque::new(),
            sub: None,
            incoming: rx,
            incoming_tx: tx,
        }
    }
}

pub fn draw(ui: &mut egui::Ui, state: &mut PubsubState, api: Arc<ApiClient>, rt: &Handle) {
    drain(state);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("topic").weak());
        ui.add(
            egui::TextEdit::singleline(&mut state.topic_input)
                .desired_width(420.0)
                .hint_text("32-byte hex"),
        );
        if state.sub.is_none() {
            if ui.button("Subscribe (PSS)").clicked() {
                start(state, &api, rt);
            }
        } else if ui.button("Unsubscribe").clicked() {
            stop(state);
        }
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("filter").weak());
        ui.add(
            egui::TextEdit::singleline(&mut state.filter_input)
                .desired_width(240.0)
                .hint_text("substring (case-insensitive)"),
        );
        if !state.messages.is_empty() && ui.button("Clear messages").clicked() {
            state.messages.clear();
        }
    });
    if let Some(sub) = &state.sub {
        ui.label(
            egui::RichText::new(format!("subscribed to {}", sub.topic_hex))
                .italics()
                .weak(),
        );
    }
    if let Some(err) = &state.error {
        ui.label(egui::RichText::new(err).color(egui::Color32::RED).small());
    }
    ui.add_space(8.0);

    let filter = if state.filter_input.trim().is_empty() {
        None
    } else {
        Some(state.filter_input.as_str())
    };
    let view = view_for(state.messages.iter().rev(), filter);

    ui.label(egui::RichText::new(format!("messages {}", view.rows.len())).strong());
    if view.rows.is_empty() {
        ui.label(
            egui::RichText::new(if state.sub.is_some() {
                "waiting for messages…"
            } else {
                "not subscribed."
            })
            .italics()
            .weak(),
        );
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("pubsub")
            .num_columns(4)
            .spacing([12.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("time").strong());
                ui.label(egui::RichText::new("kind").strong());
                ui.label(egui::RichText::new("bytes").strong());
                ui.label(egui::RichText::new("preview").strong());
                ui.end_row();
                for row in &view.rows {
                    draw_row(ui, row);
                    ui.end_row();
                }
            });
    });
}

fn draw_row(ui: &mut egui::Ui, row: &PubsubRowView) {
    ui.label(egui::RichText::new(&row.time_label).monospace().weak());
    ui.label(egui::RichText::new(row.kind_label).strong());
    ui.label(egui::RichText::new(row.payload_bytes.to_string()).monospace());
    ui.label(egui::RichText::new(&row.preview_short).monospace());
}

fn start(state: &mut PubsubState, api: &Arc<ApiClient>, rt: &Handle) {
    state.error = None;
    let trimmed = state.topic_input.trim().trim_start_matches("0x");
    let bytes = match decode_hex(trimmed, 32) {
        Ok(b) => b,
        Err(e) => {
            state.error = Some(format!("topic: {e}"));
            return;
        }
    };
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    let topic = match Topic::new(&arr) {
        Ok(t) => t,
        Err(e) => {
            state.error = Some(format!("topic: {e}"));
            return;
        }
    };
    let topic_hex = topic.to_hex();
    let cancel = CancellationToken::new();
    let tx = state.incoming_tx.clone();
    let api_c = api.clone();
    let cancel_c = cancel.clone();
    let history = None;
    rt.spawn(async move {
        if let Err(e) = spawn_pss_watcher(api_c, topic, cancel_c, tx, history).await {
            tracing::warn!("pss subscribe failed: {e}");
        }
    });
    state.sub = Some(Subscription { topic_hex, cancel });
}

fn stop(state: &mut PubsubState) {
    if let Some(sub) = state.sub.take() {
        sub.cancel.cancel();
    }
}

fn drain(state: &mut PubsubState) {
    while let Ok(msg) = state.incoming.try_recv() {
        if state.messages.len() >= MESSAGES_CAP {
            state.messages.pop_front();
        }
        state.messages.push_back(msg);
    }
}

fn decode_hex(s: &str, expected_bytes: usize) -> Result<Vec<u8>, String> {
    if s.len() != expected_bytes * 2 {
        return Err(format!(
            "expected {} hex chars, got {}",
            expected_bytes * 2,
            s.len()
        ));
    }
    let mut out = Vec::with_capacity(expected_bytes);
    for i in 0..expected_bytes {
        let byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|e| format!("invalid hex at {i}: {e}"))?;
        out.push(byte);
    }
    Ok(out)
}
