//! S6 Peers screen. Renders [`bee_cockpit_core::views::peers::view_for`]
//! plus a per-peer drill panel triggered by click or Enter.

use std::sync::Arc;

use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::views::peers::{
    BinSaturation, BinStripRow, DrillField, PeerDrillFetch, PeerDrillView, PeerRow, PeersView,
    compute_peer_drill_view, view_for,
};
use bee_cockpit_core::watch::BeeWatch;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
enum DrillState {
    Idle,
    Loading {
        peer: String,
        bin: Option<u8>,
    },
    Loaded {
        view: Box<PeerDrillView>,
    },
}

pub struct PeersScreenState {
    selected: usize,
    drill: DrillState,
    incoming: mpsc::UnboundedReceiver<(String, Option<u8>, PeerDrillFetch)>,
    incoming_tx: mpsc::UnboundedSender<(String, Option<u8>, PeerDrillFetch)>,
}

impl Default for PeersScreenState {
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

impl PeersScreenState {
    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }
    pub fn move_down(&mut self, n: usize) {
        if n > 0 && self.selected + 1 < n {
            self.selected += 1;
        }
    }
    pub fn close_drill(&mut self) -> bool {
        if matches!(self.drill, DrillState::Loaded { .. } | DrillState::Loading { .. }) {
            self.drill = DrillState::Idle;
            true
        } else {
            false
        }
    }
    pub fn drill_open(&self) -> bool {
        !matches!(self.drill, DrillState::Idle)
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    watch: &BeeWatch,
    state: &mut PeersScreenState,
    api: Arc<ApiClient>,
    rt: &Handle,
) {
    drain(state);

    let topology = watch.topology().borrow().clone();
    let Some(view) = view_for(&topology) else {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.label(
                egui::RichText::new("topology not yet loaded")
                    .italics()
                    .weak(),
            );
            if let Some(err) = &topology.last_error {
                ui.label(egui::RichText::new(err).color(egui::Color32::RED));
            }
        });
        return;
    };

    if state.selected >= view.peers.len().max(1) {
        state.selected = view.peers.len().saturating_sub(1);
    }

    handle_keys(ui, state, &view, &api, rt);

    draw_header(ui, &view);
    ui.add_space(8.0);
    draw_bins(ui, &view.bins);
    ui.add_space(8.0);

    if state.drill_open() {
        ui.columns(2, |cols| {
            draw_peers(&mut cols[0], &view.peers, state, &api, rt);
            draw_drill(&mut cols[1], state);
        });
    } else {
        draw_peers(ui, &view.peers, state, &api, rt);
        if let Some(row) = view.peers.get(state.selected) {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "selected: {}  bin {}  (Enter / click to drill)",
                    row.peer_full, row.bin
                ))
                .weak()
                .small(),
            );
        }
    }
}

fn handle_keys(
    ui: &mut egui::Ui,
    state: &mut PeersScreenState,
    view: &PeersView,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    if ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }
    let n = view.peers.len();
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
    if esc && state.close_drill() {
        return;
    }
    if up {
        state.move_up();
    }
    if down {
        state.move_down(n);
    }
    if enter {
        start_drill(state, view, api, rt);
    }
}

fn draw_header(ui: &mut egui::Ui, view: &PeersView) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("depth {}", view.depth)).strong());
        ui.label(egui::RichText::new(format!("connected {}", view.connected)).monospace());
        ui.label(egui::RichText::new(format!("population {}", view.population)).monospace());
        ui.label(egui::RichText::new(format!("light {}", view.light_connected)).monospace());
        ui.label(
            egui::RichText::new(format!(
                "starving {} · over {}",
                view.saturation.starving, view.saturation.over
            ))
            .monospace(),
        );
        if !view.reachability.is_empty() {
            ui.label(egui::RichText::new(&view.reachability).weak());
        }
    });
}

fn draw_bins(ui: &mut egui::Ui, bins: &[BinStripRow]) {
    ui.label(egui::RichText::new("Bins").strong());
    egui::Grid::new("bins")
        .num_columns(4)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("bin").strong());
            ui.label(egui::RichText::new("connected").strong());
            ui.label(egui::RichText::new("population").strong());
            ui.label(egui::RichText::new("status").strong());
            ui.end_row();
            for b in bins {
                let text = egui::RichText::new(format!("{:>2}", b.bin)).monospace();
                ui.label(if b.is_relevant { text.strong() } else { text });
                ui.label(egui::RichText::new(b.connected.to_string()).monospace());
                ui.label(egui::RichText::new(b.population.to_string()).monospace());
                ui.label(
                    egui::RichText::new(saturation_label(b.status))
                        .color(saturation_color(b.status)),
                );
                ui.end_row();
            }
        });
}

fn draw_peers(
    ui: &mut egui::Ui,
    peers: &[PeerRow],
    state: &mut PeersScreenState,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    ui.label(egui::RichText::new(format!("Peers ({})", peers.len())).strong());
    egui::ScrollArea::vertical()
        .id_salt("peers")
        .show(ui, |ui| {
            for (i, p) in peers.iter().enumerate() {
                let row_response = draw_peer_row(ui, p, i == state.selected);
                if row_response.clicked() {
                    state.selected = i;
                    start_drill_for(state, p, api, rt);
                }
                if row_response.double_clicked() {
                    state.selected = i;
                    start_drill_for(state, p, api, rt);
                }
            }
        });
}

fn draw_peer_row(ui: &mut egui::Ui, p: &PeerRow, selected: bool) -> egui::Response {
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
                ui.label(egui::RichText::new(format!("{:>2}", p.bin)).monospace());
                ui.label(egui::RichText::new(&p.peer_short).monospace());
                ui.label(egui::RichText::new(p.direction).monospace());
                let lat = egui::RichText::new(&p.latency).monospace();
                ui.label(if p.healthy {
                    lat
                } else {
                    lat.color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a))
                });
                ui.label(egui::RichText::new(&p.reachability).weak());
            });
        })
        .response;
    resp.interact(egui::Sense::click())
}

fn draw_drill(ui: &mut egui::Ui, state: &PeersScreenState) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        match &state.drill {
            DrillState::Idle => {}
            DrillState::Loading { peer, bin } => {
                ui.label(egui::RichText::new("Loading drill…").strong());
                ui.label(egui::RichText::new(peer).monospace().weak().small());
                if let Some(b) = bin {
                    ui.label(format!("bin {b}"));
                }
                ui.label(egui::RichText::new("(Esc to close)").italics().weak().small());
            }
            DrillState::Loaded { view } => draw_drill_view(ui, view),
        }
    });
}

fn draw_drill_view(ui: &mut egui::Ui, view: &PeerDrillView) {
    ui.label(egui::RichText::new("Peer drill").strong());
    ui.label(egui::RichText::new(&view.peer_overlay).monospace().small());
    if let Some(b) = view.bin {
        ui.label(format!("bin {b}"));
    }
    ui.separator();
    egui::Grid::new("drill")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            row(ui, "balance", &view.balance, |v| v.clone());
            row(ui, "ping", &view.ping, |v| v.clone());
            row(ui, "settl. received", &view.settlement_received, |v| v.clone());
            row(ui, "settl. sent", &view.settlement_sent, |v| v.clone());
            row(ui, "last cheque in", &view.last_received_cheque, |v| {
                v.clone().unwrap_or_else(|| "—".into())
            });
            row(ui, "last cheque out", &view.last_sent_cheque, |v| {
                v.clone().unwrap_or_else(|| "—".into())
            });
            row(ui, "storage radius", &view.storage_radius, |v| v.clone());
            row(ui, "reserve size", &view.reserve_size, |v| v.clone());
            row(ui, "pullsync rate", &view.pullsync_rate, |v| v.clone());
            row(ui, "batch commit.", &view.batch_commitment, |v| {
                if v.outlier {
                    format!("{} (>5% outlier)", v.formatted)
                } else {
                    v.formatted.clone()
                }
            });
        });
    ui.separator();
    ui.label(egui::RichText::new("Esc · close drill").italics().weak().small());
}

fn row<T: Clone + PartialEq + Eq, F: FnOnce(&T) -> String>(
    ui: &mut egui::Ui,
    label: &str,
    field: &DrillField<T>,
    fmt: F,
) {
    ui.label(egui::RichText::new(label).weak());
    match field {
        DrillField::Ok(v) => {
            ui.label(egui::RichText::new(fmt(v)).monospace());
        }
        DrillField::Err(e) => {
            ui.label(egui::RichText::new(format!("err: {e}")).color(egui::Color32::RED).small());
        }
    }
    ui.end_row();
}

fn start_drill(state: &mut PeersScreenState, view: &PeersView, api: &Arc<ApiClient>, rt: &Handle) {
    let Some(row) = view.peers.get(state.selected) else {
        return;
    };
    start_drill_for(state, row, api, rt);
}

fn start_drill_for(
    state: &mut PeersScreenState,
    row: &PeerRow,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    let peer = row.peer_full.clone();
    let bin = Some(row.bin);
    if let DrillState::Loading { peer: p, .. } = &state.drill {
        if *p == peer {
            return;
        }
    }
    state.drill = DrillState::Loading {
        peer: peer.clone(),
        bin,
    };
    let tx = state.incoming_tx.clone();
    let api = api.clone();
    rt.spawn(async move {
        let bee = api.bee();
        let debug = bee.debug();
        let (balance, cheques, settlement, ping, status_peers, local_status) = tokio::join!(
            debug.peer_balance(&peer),
            debug.peer_cheques(&peer),
            debug.peer_settlement(&peer),
            debug.ping_peer(&peer),
            debug.status_peers(),
            debug.status(),
        );
        let peer_status = status_peers
            .map(|rows| {
                rows.into_iter()
                    .find(|r| peer.contains(&r.status.overlay))
            })
            .map_err(|e| e.to_string());
        let fetch = PeerDrillFetch {
            balance: balance.map_err(|e| e.to_string()),
            cheques: cheques.map_err(|e| e.to_string()),
            settlement: settlement.map_err(|e| e.to_string()),
            ping: ping.map_err(|e| e.to_string()),
            peer_status,
            local_status: local_status.map_err(|e| e.to_string()),
        };
        let _ = tx.send((peer, bin, fetch));
    });
}

fn drain(state: &mut PeersScreenState) {
    while let Ok((peer, bin, fetch)) = state.incoming.try_recv() {
        let pending = match &state.drill {
            DrillState::Loading { peer: p, .. } => p.clone(),
            _ => continue,
        };
        if pending != peer {
            continue;
        }
        let view = compute_peer_drill_view(&peer, bin, &fetch);
        state.drill = DrillState::Loaded {
            view: Box::new(view),
        };
    }
}

fn saturation_label(s: BinSaturation) -> &'static str {
    match s {
        BinSaturation::Empty => "empty",
        BinSaturation::Starving => "starving",
        BinSaturation::Healthy => "healthy",
        BinSaturation::Over => "over",
    }
}

fn saturation_color(s: BinSaturation) -> egui::Color32 {
    match s {
        BinSaturation::Empty => egui::Color32::DARK_GRAY,
        BinSaturation::Starving => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        BinSaturation::Healthy => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
        BinSaturation::Over => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
    }
}
