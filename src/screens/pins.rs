//! S10 Pins screen. Mirrors bee-tui's S10: ↑/↓/j/k selection,
//! `Enter` checks the highlighted pin, `c` checks all pins, `s`
//! cycles sort mode. Click = select + check.

use std::collections::HashMap;
use std::sync::Arc;

use bee::api::PinIntegrity;
use bee::swarm::Reference;
use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::views::pins::{CheckState, PinRow, SortMode, view_for};
use bee_cockpit_core::watch::BeeWatch;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

type FetchResult = (Reference, std::result::Result<PinIntegrity, String>);

pub struct PinsScreenState {
    selected: usize,
    sort: SortMode,
    checks: HashMap<Reference, CheckState>,
    incoming: mpsc::UnboundedReceiver<FetchResult>,
    incoming_tx: mpsc::UnboundedSender<FetchResult>,
}

impl Default for PinsScreenState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            selected: 0,
            sort: SortMode::Reference,
            checks: HashMap::new(),
            incoming: rx,
            incoming_tx: tx,
        }
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    watch: &BeeWatch,
    state: &mut PinsScreenState,
    api: Arc<ApiClient>,
    rt: &Handle,
) {
    drain(state);

    let snap = watch.pins().borrow().clone();
    let view = view_for(&snap, &state.checks, state.sort);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("total {}", view.total_pins)).strong());
        ui.label(
            egui::RichText::new(format!("healthy {}", view.healthy))
                .color(egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)),
        );
        ui.label(
            egui::RichText::new(format!("unhealthy {}", view.unhealthy))
                .color(egui::Color32::from_rgb(0xd0, 0x4a, 0x4a)),
        );
        ui.label(egui::RichText::new(format!("unchecked {}", view.unchecked)).weak());
        ui.label(egui::RichText::new(format!("· sort: {}", view.sort.label())).weak());
    });
    ui.horizontal(|ui| {
        if ui
            .button("Check selected")
            .on_hover_text("Enter")
            .clicked()
        {
            check_selected(state, &snap.pins, &api, rt);
        }
        if ui.button("Check all").on_hover_text("c").clicked() {
            check_all(state, &snap.pins, &api, rt);
        }
        if ui
            .button(format!("Sort: {}", state.sort.label()))
            .on_hover_text("s — cycle sort mode")
            .clicked()
        {
            state.sort = state.sort.next();
        }
    });
    ui.add_space(8.0);

    if view.rows.is_empty() {
        ui.label(egui::RichText::new("(no pins)").italics().weak());
        return;
    }

    let n = view.rows.len();
    handle_keys(ui, state, &snap.pins, n, &api, rt);
    if state.selected >= n {
        state.selected = n.saturating_sub(1);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, row) in view.rows.iter().enumerate() {
            let resp = draw_row(ui, row, i == state.selected);
            if resp.clicked() {
                state.selected = i;
                check_selected(state, &snap.pins, &api, rt);
            }
        }
    });
}

fn handle_keys(
    ui: &mut egui::Ui,
    state: &mut PinsScreenState,
    pins: &[Reference],
    n: usize,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    if ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }
    let mut up = false;
    let mut down = false;
    let mut page_up = false;
    let mut page_down = false;
    let mut enter = false;
    let mut check_all_k = false;
    let mut sort_k = false;
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
        if i.key_pressed(egui::Key::C) {
            check_all_k = true;
        }
        if i.key_pressed(egui::Key::S) {
            sort_k = true;
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
    if enter {
        check_selected(state, pins, api, rt);
    }
    if check_all_k {
        check_all(state, pins, api, rt);
    }
    if sort_k {
        state.sort = state.sort.next();
    }
}

fn draw_row(ui: &mut egui::Ui, row: &PinRow, selected: bool) -> egui::Response {
    let bg = if selected {
        egui::Color32::from_rgb(0x3a, 0x6a, 0x9c)
    } else {
        egui::Color32::TRANSPARENT
    };
    let mut frame = egui::Frame::none().fill(bg);
    frame.inner_margin = egui::Margin::symmetric(4.0, 1.0);
    let (label, color) = match &row.check {
        CheckState::Idle => ("unchecked".into(), egui::Color32::GRAY),
        CheckState::Checking => ("checking…".into(), egui::Color32::from_rgb(0xe0, 0xb0, 0x30)),
        CheckState::Ok {
            total,
            missing,
            invalid,
        } if *missing == 0 && *invalid == 0 => (
            format!("ok · {total} chunks"),
            if *total > 0 {
                egui::Color32::from_rgb(0x4a, 0xc0, 0x4a)
            } else {
                egui::Color32::GRAY
            },
        ),
        CheckState::Ok {
            total,
            missing,
            invalid,
        } => (
            format!("unhealthy · total {total} · missing {missing} · invalid {invalid}"),
            egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        ),
        CheckState::Failed(e) => (
            format!("error · {e}"),
            egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        ),
    };
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&row.reference_short).monospace());
                ui.label(egui::RichText::new(label).color(color));
            });
        })
        .response;
    resp.interact(egui::Sense::click())
}

fn check_selected(
    state: &mut PinsScreenState,
    pins: &[Reference],
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    let Some(reference) = pins.get(state.selected).cloned() else {
        return;
    };
    spawn_check(state, reference, api, rt);
}

fn check_all(
    state: &mut PinsScreenState,
    pins: &[Reference],
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    let pending: Vec<Reference> = pins
        .iter()
        .filter(|r| matches!(state.checks.get(*r), None | Some(CheckState::Idle)))
        .cloned()
        .collect();
    for r in pending {
        spawn_check(state, r, api, rt);
    }
}

fn spawn_check(
    state: &mut PinsScreenState,
    reference: Reference,
    api: &Arc<ApiClient>,
    rt: &Handle,
) {
    if matches!(state.checks.get(&reference), Some(CheckState::Checking)) {
        return;
    }
    state.checks.insert(reference.clone(), CheckState::Checking);
    let api = api.clone();
    let tx = state.incoming_tx.clone();
    rt.spawn(async move {
        let r = api
            .bee()
            .api()
            .check_pins(Some(&reference))
            .await
            .map_err(|e| e.to_string())
            .and_then(|mut entries| {
                entries
                    .pop()
                    .ok_or_else(|| "Bee returned no integrity entry".to_string())
            });
        let _ = tx.send((reference, r));
    });
}

fn drain(state: &mut PinsScreenState) {
    while let Ok((reference, result)) = state.incoming.try_recv() {
        let next = match result {
            Ok(p) => CheckState::Ok {
                total: p.total,
                missing: p.missing,
                invalid: p.invalid,
            },
            Err(e) => CheckState::Failed(e),
        };
        state.checks.insert(reference, next);
    }
}
