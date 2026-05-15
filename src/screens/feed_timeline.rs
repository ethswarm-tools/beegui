//! S13 Feed timeline screen.

use std::sync::Arc;

use bee::swarm::{EthAddress, Topic};
use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::feed_timeline::{Timeline, walk};
use bee_cockpit_core::views::feed_timeline::{FeedRowView, view_for};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

pub struct FeedTimelineState {
    owner_input: String,
    topic_input: String,
    max_entries_input: String,
    error: Option<String>,
    timeline: Option<Timeline>,
    inflight: bool,
    incoming: mpsc::UnboundedReceiver<Result<Timeline, String>>,
    incoming_tx: mpsc::UnboundedSender<Result<Timeline, String>>,
}

impl Default for FeedTimelineState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            owner_input: String::new(),
            topic_input: String::new(),
            max_entries_input: "20".into(),
            error: None,
            timeline: None,
            inflight: false,
            incoming: rx,
            incoming_tx: tx,
        }
    }
}

impl FeedTimelineState {
    pub fn load_external(
        &mut self,
        owner: String,
        topic: String,
        max: Option<u64>,
        api: &Arc<ApiClient>,
        rt: &Handle,
    ) {
        self.owner_input = owner;
        self.topic_input = topic;
        if let Some(m) = max {
            self.max_entries_input = m.to_string();
        }
        start(self, api, rt);
    }
}

pub fn draw(ui: &mut egui::Ui, state: &mut FeedTimelineState, api: Arc<ApiClient>, rt: &Handle) {
    drain(state);

    egui::Grid::new("feed-input")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("owner").weak());
            ui.add(
                egui::TextEdit::singleline(&mut state.owner_input)
                    .desired_width(420.0)
                    .hint_text("20-byte hex (Ethereum address)"),
            );
            ui.end_row();
            ui.label(egui::RichText::new("topic").weak());
            ui.add(
                egui::TextEdit::singleline(&mut state.topic_input)
                    .desired_width(420.0)
                    .hint_text("32-byte hex"),
            );
            ui.end_row();
            ui.label(egui::RichText::new("max entries").weak());
            ui.add(
                egui::TextEdit::singleline(&mut state.max_entries_input).desired_width(60.0),
            );
            ui.end_row();
        });

    ui.horizontal(|ui| {
        let walk_btn = ui.button("Walk feed");
        if walk_btn.clicked() {
            start(state, &api, rt);
        }
        if state.inflight {
            ui.label(egui::RichText::new("walking…").italics().weak());
        }
    });
    if let Some(err) = &state.error {
        ui.label(egui::RichText::new(err).color(egui::Color32::RED).small());
    }
    ui.add_space(8.0);

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let view = view_for(state.timeline.as_ref(), now_unix);

    if let Some(h) = &view.header {
        ui.label(
            egui::RichText::new(format!(
                "owner {} · topic {} · latest idx {} · {} entries",
                h.owner_hex_short, h.topic_hex_short, h.latest_index, h.entry_count
            ))
            .strong(),
        );
        ui.add_space(4.0);
    }
    if view.rows.is_empty() {
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("feed-rows")
            .num_columns(5)
            .spacing([12.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("idx").strong());
                ui.label(egui::RichText::new("age").strong());
                ui.label(egui::RichText::new("size").strong());
                ui.label(egui::RichText::new("kind").strong());
                ui.label(egui::RichText::new("body").strong());
                ui.end_row();
                for row in &view.rows {
                    draw_row(ui, row);
                    ui.end_row();
                }
            });
    });
}

fn draw_row(ui: &mut egui::Ui, row: &FeedRowView) {
    let dim = row.is_error;
    let style = |t: egui::RichText| if dim { t.weak() } else { t };
    ui.label(style(egui::RichText::new(row.index.to_string()).monospace()));
    ui.label(style(egui::RichText::new(&row.age_label).monospace()));
    ui.label(style(egui::RichText::new(&row.size_label).monospace()));
    let kind_color = match row.kind {
        "miss" => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        "ref" => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        _ => egui::Color32::GRAY,
    };
    ui.label(egui::RichText::new(row.kind).color(kind_color).monospace());
    ui.label(style(egui::RichText::new(&row.body).monospace()));
}

fn start(state: &mut FeedTimelineState, api: &Arc<ApiClient>, rt: &Handle) {
    state.error = None;
    let owner = match parse_eth(&state.owner_input) {
        Ok(o) => o,
        Err(e) => {
            state.error = Some(format!("owner: {e}"));
            return;
        }
    };
    let topic = match parse_topic(&state.topic_input) {
        Ok(t) => t,
        Err(e) => {
            state.error = Some(format!("topic: {e}"));
            return;
        }
    };
    let max_entries: u64 = state
        .max_entries_input
        .trim()
        .parse()
        .unwrap_or(20)
        .max(1);
    state.inflight = true;
    let api = api.clone();
    let tx = state.incoming_tx.clone();
    rt.spawn(async move {
        let r = walk(api, owner, topic, max_entries).await;
        let _ = tx.send(r);
    });
}

fn drain(state: &mut FeedTimelineState) {
    while let Ok(msg) = state.incoming.try_recv() {
        state.inflight = false;
        match msg {
            Ok(t) => state.timeline = Some(t),
            Err(e) => state.error = Some(e),
        }
    }
}

fn parse_eth(s: &str) -> Result<EthAddress, String> {
    let trimmed = s.trim().trim_start_matches("0x");
    let bytes = decode_hex(trimmed, 20)?;
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    EthAddress::new(&arr).map_err(|e| e.to_string())
}

fn parse_topic(s: &str) -> Result<Topic, String> {
    let trimmed = s.trim().trim_start_matches("0x");
    let bytes = decode_hex(trimmed, 32)?;
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Topic::new(&arr).map_err(|e| e.to_string())
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
