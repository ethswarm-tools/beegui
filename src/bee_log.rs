//! External Bee log tailing — beegui side of bee-cockpit-core's
//! `bee_log_tailer` + `bee_log_discover`. Owns the per-tab ring
//! buffers the bottom log pane reads from.
//!
//! Data flow:
//!
//! ```text
//! tailer (file / command / discovery)
//!   --(LogTab, BeeLogLine)-->  mpsc channel
//!     --drain()-->  BeeLogs::rings[tab]  --snapshot()-->  draw_log_pane
//! ```
//!
//! The bee::http tab (SelfHttp) and the Cockpit tab read from the
//! existing `log_capture::LogCapture` / `CockpitCapture` globals
//! directly — only Errors / Warning / Info / Debug / BeeHttp are
//! populated by the tailer.

use std::collections::VecDeque;
use std::path::PathBuf;

use bee_cockpit_core::bee_log::{BeeLogLine, LogTab};
use bee_cockpit_core::bee_log_discover::{self, BeeLogSource, DiscoveryResult};
use bee_cockpit_core::bee_log_tailer;
use bee_cockpit_core::config::NodeConfig;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Per-tab capacity. Bee can be chatty under load (peer churn,
/// reserve writes) so the rings are generous but bounded.
const RING_CAPACITY: usize = 1000;

/// Receiver-side ring buffers + the live mpsc channel from the
/// tailer. One `BeeLogs` per beegui App; tailers come and go with
/// node switches.
pub struct BeeLogs {
    rings: [VecDeque<BeeLogLine>; 5],
    tx: mpsc::UnboundedSender<(LogTab, BeeLogLine)>,
    rx: mpsc::UnboundedReceiver<(LogTab, BeeLogLine)>,
    /// What source — if any — is currently being tailed. Pure
    /// status info for the pane header / banner; doesn't gate
    /// any behaviour.
    pub source: ResolvedSource,
}

/// Snapshot of *why* a given tail-source landed where it did —
/// surfaced in the pane header so operators understand whether the
/// Bee-side tabs are empty for a configurable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSource {
    /// No source resolved. Bee-side tabs stay empty; the pane shows
    /// the placeholder hint.
    None { reason: String },
    /// A file is being tailed (CLI flag / config / discovered).
    File { path: PathBuf, origin: SourceOrigin },
    /// A command is being tailed.
    Command {
        command: String,
        origin: SourceOrigin,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    Cli,
    Config,
    Discovery,
}

impl SourceOrigin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cli => "CLI",
            Self::Config => "config",
            Self::Discovery => "discovered",
        }
    }
}

impl Default for BeeLogs {
    fn default() -> Self {
        Self::new()
    }
}

impl BeeLogs {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            rings: Default::default(),
            tx,
            rx,
            source: ResolvedSource::None {
                reason: "no source resolved yet".into(),
            },
        }
    }

    /// Drain pending lines from the tailer into the per-tab rings.
    /// Called every frame.
    pub fn drain(&mut self) {
        while let Ok((tab, line)) = self.rx.try_recv() {
            let Some(idx) = ring_index(tab) else {
                continue;
            };
            let ring = &mut self.rings[idx];
            if ring.len() >= RING_CAPACITY {
                ring.pop_front();
            }
            ring.push_back(line);
        }
    }

    pub fn snapshot(&self, tab: LogTab) -> &VecDeque<BeeLogLine> {
        static EMPTY: VecDeque<BeeLogLine> = VecDeque::new();
        match ring_index(tab) {
            Some(i) => &self.rings[i],
            None => &EMPTY,
        }
    }

    /// Tear down the current tailer (the previous `CancellationToken`
    /// is dropped) and spawn a new one for `source`.
    pub fn respawn(&mut self, source: ResolvedSource, cancel: CancellationToken) {
        match &source {
            ResolvedSource::File { path, .. } => {
                bee_log_tailer::spawn(path.clone(), self.tx.clone(), cancel, true);
            }
            ResolvedSource::Command { command, .. } => {
                bee_log_tailer::spawn_command(command.clone(), self.tx.clone(), cancel);
            }
            ResolvedSource::None { .. } => {}
        }
        // Clear the rings on respawn so stale lines from the
        // previous node don't bleed into the new node's tabs.
        for ring in &mut self.rings {
            ring.clear();
        }
        self.source = source;
    }
}

fn ring_index(tab: LogTab) -> Option<usize> {
    match tab {
        LogTab::Errors => Some(0),
        LogTab::Warning => Some(1),
        LogTab::Info => Some(2),
        LogTab::Debug => Some(3),
        LogTab::BeeHttp => Some(4),
        LogTab::SelfHttp | LogTab::Cockpit => None,
    }
}

/// Resolve a tail source from (CLI flag, active node's config,
/// discovery). Returns a `ResolvedSource` carrying both the
/// effective source and the reason it was picked — used for the
/// pane header and the "(no source)" hint.
pub fn resolve_source(
    cli_log_file: Option<&str>,
    cli_log_command: Option<&str>,
    node: &NodeConfig,
) -> ResolvedSource {
    if let Some(p) = cli_log_file {
        return ResolvedSource::File {
            path: PathBuf::from(p),
            origin: SourceOrigin::Cli,
        };
    }
    if let Some(c) = cli_log_command {
        return ResolvedSource::Command {
            command: c.to_string(),
            origin: SourceOrigin::Cli,
        };
    }
    if let Some(p) = &node.log_file {
        return ResolvedSource::File {
            path: PathBuf::from(p),
            origin: SourceOrigin::Config,
        };
    }
    if let Some(c) = &node.log_command {
        return ResolvedSource::Command {
            command: c.clone(),
            origin: SourceOrigin::Config,
        };
    }
    match bee_log_discover::discover(&node.url) {
        DiscoveryResult::Found(BeeLogSource::File(path)) => ResolvedSource::File {
            path,
            origin: SourceOrigin::Discovery,
        },
        DiscoveryResult::Found(BeeLogSource::Command(command)) => ResolvedSource::Command {
            command,
            origin: SourceOrigin::Discovery,
        },
        DiscoveryResult::Unsupported(msg) => ResolvedSource::None { reason: msg },
        DiscoveryResult::NotApplicable => ResolvedSource::None {
            reason: "no --bee-log / [bee].log_file / log_command configured; \
                     auto-discovery only runs on Linux against local Bee nodes"
                .into(),
        },
    }
}
