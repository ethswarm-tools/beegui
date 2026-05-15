//! S11 Manifest screen. Operator pastes a root reference, beegui
//! lazily fetches the root chunk + each expanded fork via
//! [`bee_cockpit_core::manifest_walker`].

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bee::swarm::Reference;
use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::manifest_walker;
use bee_cockpit_core::views::manifest::{NodeState, TreeRow, parse_hex_32, view_for};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

/// Renderer-local state for the manifest walker.
pub struct ManifestState {
    input: String,
    error: Option<String>,
    root_ref: Option<Reference>,
    root: NodeState,
    forks: HashMap<[u8; 32], NodeState>,
    expanded: HashSet<[u8; 32]>,
    inflight_root: bool,
    inflight_forks: HashSet<[u8; 32]>,
    selected: usize,
    incoming: mpsc::UnboundedReceiver<WalkResult>,
    incoming_tx: mpsc::UnboundedSender<WalkResult>,
}

enum WalkResult {
    Root(Result<bee::manifest::MantarayNode, String>),
    Fork([u8; 32], Result<bee::manifest::MantarayNode, String>),
}

impl Default for ManifestState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            input: String::new(),
            error: None,
            root_ref: None,
            root: NodeState::Idle,
            forks: HashMap::new(),
            expanded: HashSet::new(),
            inflight_root: false,
            inflight_forks: HashSet::new(),
            selected: 0,
            incoming: rx,
            incoming_tx: tx,
        }
    }
}

impl ManifestState {
    /// Programmatic load triggered by the palette's `:inspect <ref>`.
    pub fn load_external(&mut self, reference: String, api: &Arc<ApiClient>, rt: &Handle) {
        self.input = reference;
        start_load(self, api, rt);
    }
}

pub fn draw(ui: &mut egui::Ui, state: &mut ManifestState, api: Arc<ApiClient>, rt: &Handle) {
    drain_results(state);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("root ref").weak());
        let r = ui.add(
            egui::TextEdit::singleline(&mut state.input)
                .desired_width(420.0)
                .hint_text("paste a 32-byte hex reference"),
        );
        let load = ui.button("Load");
        if load.clicked()
            || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
        {
            start_load(state, &api, rt);
        }
        if !state.input.is_empty() && ui.button("Clear").clicked() {
            *state = ManifestState::default();
        }
    });
    if let Some(err) = &state.error {
        ui.label(egui::RichText::new(err).color(egui::Color32::RED).small());
    }
    ui.add_space(8.0);

    let view = view_for(
        state.root_ref.as_ref(),
        &state.root,
        &state.forks,
        &state.expanded,
    );
    ui.label(egui::RichText::new(&view.header).strong());
    ui.add_space(4.0);

    if view.rows.is_empty() {
        return;
    }

    let n = view.rows.len();
    if !ui.ctx().memory(|m| m.focused().is_some()) {
        let mut up = false;
        let mut down = false;
        let mut page_up = false;
        let mut page_down = false;
        let mut enter = false;
        ui.input(|i| {
            if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K) {
                up = true;
            }
            if i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J) {
                down = true;
            }
            if i.key_pressed(egui::Key::PageUp) {
                page_up = true;
            }
            if i.key_pressed(egui::Key::PageDown) {
                page_down = true;
            }
            if i.key_pressed(egui::Key::Enter) {
                enter = true;
            }
        });
        if up {
            state.selected = state.selected.saturating_sub(1);
        }
        if down && state.selected + 1 < n {
            state.selected += 1;
        }
        if page_up {
            state.selected = state.selected.saturating_sub(10);
        }
        if page_down {
            state.selected = (state.selected + 10).min(n.saturating_sub(1));
        }
        if enter && state.selected < n {
            let row = &view.rows[state.selected];
            if row.has_children
                && let Some(hex) = &row.self_addr_hex
                && let Ok(addr) = parse_hex_32(hex)
            {
                toggle_fork(state, addr, &api, rt);
            }
        }
    }
    if state.selected >= n {
        state.selected = n.saturating_sub(1);
    }

    let mut toggles: Vec<[u8; 32]> = Vec::new();
    let mut new_selected: Option<usize> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, row) in view.rows.iter().enumerate() {
            let clicked = draw_row(ui, row, i == state.selected);
            if clicked {
                new_selected = Some(i);
                if row.has_children
                    && let Some(hex) = &row.self_addr_hex
                    && let Ok(addr) = parse_hex_32(hex)
                {
                    toggles.push(addr);
                }
            }
        }
    });
    if let Some(i) = new_selected {
        state.selected = i;
    }
    for addr in toggles {
        toggle_fork(state, addr, &api, rt);
    }
}

fn draw_row(ui: &mut egui::Ui, row: &TreeRow, selected: bool) -> bool {
    let indent = row.depth as f32 * 16.0;
    let bg = if selected {
        egui::Color32::from_rgb(0x3a, 0x6a, 0x9c)
    } else {
        egui::Color32::TRANSPARENT
    };
    let mut frame = egui::Frame::none().fill(bg);
    frame.inner_margin = egui::Margin::symmetric(2.0, 1.0);
    let mut clicked = false;
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(indent);
            let label = format!("{}  {}", row.glyph, row.label);
            let resp = ui.add(egui::Label::new(label).sense(egui::Sense::click()));
            if resp.clicked() {
                clicked = true;
            }
            if let Some(ct) = &row.content_type {
                ui.label(egui::RichText::new(ct).weak().small());
            }
            if let Some(hint) = &row.state_hint {
                ui.label(egui::RichText::new(hint).italics().weak().small());
            }
            if let Some(t) = &row.target_ref_hex {
                ui.label(egui::RichText::new(t).monospace().weak().small());
            }
        });
    });
    clicked
}

fn start_load(state: &mut ManifestState, api: &Arc<ApiClient>, rt: &Handle) {
    state.error = None;
    let trimmed = state.input.trim().trim_start_matches("0x");
    let bytes = match parse_hex_32(trimmed) {
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
    state.root_ref = Some(reference.clone());
    state.root = NodeState::Loading;
    state.forks.clear();
    state.expanded.clear();
    state.inflight_forks.clear();
    state.inflight_root = true;
    let tx = state.incoming_tx.clone();
    let api = api.clone();
    rt.spawn(async move {
        let r = manifest_walker::load_node(api, reference).await;
        let _ = tx.send(WalkResult::Root(r));
    });
}

fn toggle_fork(state: &mut ManifestState, addr: [u8; 32], api: &Arc<ApiClient>, rt: &Handle) {
    if state.expanded.contains(&addr) {
        state.expanded.remove(&addr);
        return;
    }
    state.expanded.insert(addr);
    if state.forks.contains_key(&addr)
        && matches!(
            state.forks.get(&addr),
            Some(NodeState::Loaded(_)) | Some(NodeState::Error(_))
        )
    {
        // Already have data — toggle is enough.
        return;
    }
    if state.inflight_forks.contains(&addr) {
        return;
    }
    let Ok(reference) = Reference::new(&addr) else {
        state
            .forks
            .insert(addr, NodeState::Error("bad self address".into()));
        return;
    };
    state.forks.insert(addr, NodeState::Loading);
    state.inflight_forks.insert(addr);
    let tx = state.incoming_tx.clone();
    let api = api.clone();
    rt.spawn(async move {
        let r = manifest_walker::load_node(api, reference).await;
        let _ = tx.send(WalkResult::Fork(addr, r));
    });
}

fn drain_results(state: &mut ManifestState) {
    while let Ok(msg) = state.incoming.try_recv() {
        match msg {
            WalkResult::Root(r) => {
                state.inflight_root = false;
                state.root = match r {
                    Ok(node) => NodeState::Loaded(Box::new(node)),
                    Err(e) => NodeState::Error(e),
                };
            }
            WalkResult::Fork(addr, r) => {
                state.inflight_forks.remove(&addr);
                state.forks.insert(
                    addr,
                    match r {
                        Ok(node) => NodeState::Loaded(Box::new(node)),
                        Err(e) => NodeState::Error(e),
                    },
                );
            }
        }
    }
}
