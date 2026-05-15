//! S14 Pubsub screen. Subscribes via PSS (topic) or GSOC
//! (owner+identifier) and tails the message stream into a ring
//! buffer. Optional JSONL history file.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use bee::swarm::{EthAddress, Identifier, Topic};
use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::pubsub::{
    HistoryWriter, PubsubMessage, open_history_writer, spawn_gsoc_watcher, spawn_pss_watcher,
};
use bee_cockpit_core::views::pubsub::{PubsubRowView, view_for};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const MESSAGES_CAP: usize = 200;
const DEFAULT_HISTORY_ROTATE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_HISTORY_KEEP: u32 = 5;

pub struct PubsubState {
    mode: SubMode,
    topic_input: String,
    owner_input: String,
    identifier_input: String,
    filter_input: String,
    history_path_input: String,
    error: Option<String>,
    messages: VecDeque<PubsubMessage>,
    sub: Option<Subscription>,
    incoming: mpsc::UnboundedReceiver<PubsubMessage>,
    incoming_tx: mpsc::UnboundedSender<PubsubMessage>,
}

#[derive(PartialEq, Eq)]
enum SubMode {
    Pss,
    Gsoc,
}

struct Subscription {
    kind: &'static str,
    channel: String,
    history_path: Option<PathBuf>,
    cancel: CancellationToken,
}

impl Default for PubsubState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            mode: SubMode::Pss,
            topic_input: String::new(),
            owner_input: String::new(),
            identifier_input: String::new(),
            filter_input: String::new(),
            history_path_input: String::new(),
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
        ui.selectable_value(&mut state.mode, SubMode::Pss, "PSS");
        ui.selectable_value(&mut state.mode, SubMode::Gsoc, "GSOC");
    });
    ui.add_space(4.0);

    match state.mode {
        SubMode::Pss => draw_pss_inputs(ui, state),
        SubMode::Gsoc => draw_gsoc_inputs(ui, state),
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("history file").weak());
        ui.add(
            egui::TextEdit::singleline(&mut state.history_path_input)
                .desired_width(360.0)
                .hint_text("(optional) /path/to/pubsub.jsonl"),
        );
    });
    ui.horizontal(|ui| {
        if state.sub.is_none() {
            let label = match state.mode {
                SubMode::Pss => "Subscribe (PSS)",
                SubMode::Gsoc => "Subscribe (GSOC)",
            };
            if ui.button(label).clicked() {
                start(state, &api, rt);
            }
        } else if ui.button("Unsubscribe").clicked() {
            stop(state);
        }
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
        let suffix = if let Some(p) = &sub.history_path {
            format!(" · history → {}", p.display())
        } else {
            String::new()
        };
        ui.label(
            egui::RichText::new(format!(
                "subscribed via {} to {}{}",
                sub.kind, sub.channel, suffix
            ))
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

fn draw_pss_inputs(ui: &mut egui::Ui, state: &mut PubsubState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("topic").weak());
        ui.add(
            egui::TextEdit::singleline(&mut state.topic_input)
                .desired_width(420.0)
                .hint_text("32-byte hex"),
        );
    });
}

fn draw_gsoc_inputs(ui: &mut egui::Ui, state: &mut PubsubState) {
    egui::Grid::new("gsoc-input")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("owner").weak());
            ui.add(
                egui::TextEdit::singleline(&mut state.owner_input)
                    .desired_width(420.0)
                    .hint_text("20-byte hex"),
            );
            ui.end_row();
            ui.label(egui::RichText::new("identifier").weak());
            ui.add(
                egui::TextEdit::singleline(&mut state.identifier_input)
                    .desired_width(420.0)
                    .hint_text("32-byte hex"),
            );
            ui.end_row();
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
    let history_path = if state.history_path_input.trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(state.history_path_input.trim()))
    };
    let history: HistoryWriter = if let Some(p) = history_path.clone() {
        match rt.block_on(open_history_writer(
            &p,
            DEFAULT_HISTORY_ROTATE_BYTES,
            DEFAULT_HISTORY_KEEP,
        )) {
            Ok(w) => w,
            Err(e) => {
                state.error = Some(format!("history file: {e}"));
                None
            }
        }
    } else {
        None
    };

    match state.mode {
        SubMode::Pss => start_pss(state, api, rt, history, history_path),
        SubMode::Gsoc => start_gsoc(state, api, rt, history, history_path),
    }
}

fn start_pss(
    state: &mut PubsubState,
    api: &Arc<ApiClient>,
    rt: &Handle,
    history: HistoryWriter,
    history_path: Option<PathBuf>,
) {
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
    let channel = topic.to_hex();
    let cancel = CancellationToken::new();
    let tx = state.incoming_tx.clone();
    let api_c = api.clone();
    let cancel_c = cancel.clone();
    rt.spawn(async move {
        if let Err(e) = spawn_pss_watcher(api_c, topic, cancel_c, tx, history).await {
            tracing::warn!(target: "beegui::pubsub", "pss subscribe failed: {e}");
        }
    });
    state.sub = Some(Subscription {
        kind: "PSS",
        channel,
        history_path,
        cancel,
    });
}

fn start_gsoc(
    state: &mut PubsubState,
    api: &Arc<ApiClient>,
    rt: &Handle,
    history: HistoryWriter,
    history_path: Option<PathBuf>,
) {
    let owner_hex = state.owner_input.trim().trim_start_matches("0x");
    let owner_bytes = match decode_hex(owner_hex, 20) {
        Ok(b) => b,
        Err(e) => {
            state.error = Some(format!("owner: {e}"));
            return;
        }
    };
    let mut owner_arr = [0u8; 20];
    owner_arr.copy_from_slice(&owner_bytes);
    let owner = match EthAddress::new(&owner_arr) {
        Ok(o) => o,
        Err(e) => {
            state.error = Some(format!("owner: {e}"));
            return;
        }
    };
    let id_hex = state.identifier_input.trim().trim_start_matches("0x");
    let id_bytes = match decode_hex(id_hex, 32) {
        Ok(b) => b,
        Err(e) => {
            state.error = Some(format!("identifier: {e}"));
            return;
        }
    };
    let mut id_arr = [0u8; 32];
    id_arr.copy_from_slice(&id_bytes);
    let identifier = match Identifier::new(&id_arr) {
        Ok(i) => i,
        Err(e) => {
            state.error = Some(format!("identifier: {e}"));
            return;
        }
    };
    let channel = format!("{}/{}", owner.to_hex(), identifier.to_hex());
    let cancel = CancellationToken::new();
    let tx = state.incoming_tx.clone();
    let api_c = api.clone();
    let cancel_c = cancel.clone();
    rt.spawn(async move {
        if let Err(e) =
            spawn_gsoc_watcher(api_c, owner, identifier, cancel_c, tx, history).await
        {
            tracing::warn!(target: "beegui::pubsub", "gsoc subscribe failed: {e}");
        }
    });
    state.sub = Some(Subscription {
        kind: "GSOC",
        channel,
        history_path,
        cancel,
    });
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
