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
    /// Source is the file the in-process supervisor writes Bee's
    /// stdout+stderr to. Highest-priority — picked whenever the
    /// supervisor is active.
    Supervisor,
}

impl SourceOrigin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cli => "CLI",
            Self::Config => "config",
            Self::Discovery => "discovered",
            Self::Supervisor => "supervised",
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
    /// is dropped) and spawn a new one for `source`. Replaces the
    /// internal mpsc channel so in-flight messages from the dying
    /// tailer can't leak into the new rings.
    pub fn respawn(&mut self, source: ResolvedSource, cancel: CancellationToken) {
        // Fresh channel: any pending sends from the previous tailer
        // (which received its cancel signal but may have a few lines
        // still in flight in its 200 ms read loop) go to the dropped
        // receiver and are discarded — they can't appear in this
        // node's tabs.
        let (tx, rx) = mpsc::unbounded_channel();
        self.tx = tx;
        self.rx = rx;
        match &source {
            ResolvedSource::File { path, .. } => {
                bee_log_tailer::spawn(path.clone(), self.tx.clone(), cancel, true);
            }
            ResolvedSource::Command { command, .. } => {
                bee_log_tailer::spawn_command(command.clone(), self.tx.clone(), cancel);
            }
            ResolvedSource::None { .. } => {}
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use bee_cockpit_core::bee_log::{BeeLogLine, LogTab};

    fn mk_node() -> NodeConfig {
        // Use a non-loopback URL so discovery falls through to
        // NotApplicable deterministically — tests must not depend on
        // /proc state of the host they're running on.
        NodeConfig {
            name: "test".into(),
            url: "http://example.com:1633".into(),
            token: None,
            log_file: None,
            log_command: None,
            default: true,
        }
    }

    #[test]
    fn resolve_source_cli_file_wins_over_everything() {
        let mut node = mk_node();
        node.log_file = Some("/cfg/file.log".into());
        node.log_command = Some("cfg-cmd".into());
        let s = resolve_source(Some("/cli/file.log"), Some("cli-cmd"), &node);
        match s {
            ResolvedSource::File { path, origin } => {
                assert_eq!(path, PathBuf::from("/cli/file.log"));
                assert_eq!(origin, SourceOrigin::Cli);
            }
            other => panic!("expected CLI File, got {other:?}"),
        }
    }

    #[test]
    fn resolve_source_cli_cmd_wins_when_no_cli_file() {
        let mut node = mk_node();
        node.log_file = Some("/cfg/file.log".into());
        node.log_command = Some("cfg-cmd".into());
        let s = resolve_source(None, Some("cli-cmd"), &node);
        match s {
            ResolvedSource::Command { command, origin } => {
                assert_eq!(command, "cli-cmd");
                assert_eq!(origin, SourceOrigin::Cli);
            }
            other => panic!("expected CLI Command, got {other:?}"),
        }
    }

    #[test]
    fn resolve_source_config_file_wins_when_no_cli() {
        let mut node = mk_node();
        node.log_file = Some("/cfg/file.log".into());
        node.log_command = Some("cfg-cmd".into());
        let s = resolve_source(None, None, &node);
        match s {
            ResolvedSource::File { path, origin } => {
                assert_eq!(path, PathBuf::from("/cfg/file.log"));
                assert_eq!(origin, SourceOrigin::Config);
            }
            other => panic!("expected Config File, got {other:?}"),
        }
    }

    #[test]
    fn resolve_source_config_cmd_when_no_config_file() {
        let mut node = mk_node();
        node.log_command = Some("cfg-cmd".into());
        let s = resolve_source(None, None, &node);
        match s {
            ResolvedSource::Command { command, origin } => {
                assert_eq!(command, "cfg-cmd");
                assert_eq!(origin, SourceOrigin::Config);
            }
            other => panic!("expected Config Command, got {other:?}"),
        }
    }

    #[test]
    fn resolve_source_falls_through_to_none_for_remote_url() {
        let node = mk_node();
        let s = resolve_source(None, None, &node);
        match s {
            ResolvedSource::None { reason } => {
                assert!(reason.contains("auto-discovery") || reason.contains("--bee-log"));
            }
            other => panic!("expected None for remote URL, got {other:?}"),
        }
    }

    #[test]
    fn source_origin_labels_are_short() {
        for origin in [
            SourceOrigin::Cli,
            SourceOrigin::Config,
            SourceOrigin::Discovery,
            SourceOrigin::Supervisor,
        ] {
            assert!(!origin.label().is_empty());
            assert!(origin.label().len() < 16);
        }
    }

    #[test]
    fn ring_index_routes_severity_tabs() {
        assert_eq!(ring_index(LogTab::Errors), Some(0));
        assert_eq!(ring_index(LogTab::Warning), Some(1));
        assert_eq!(ring_index(LogTab::Info), Some(2));
        assert_eq!(ring_index(LogTab::Debug), Some(3));
        assert_eq!(ring_index(LogTab::BeeHttp), Some(4));
    }

    #[test]
    fn ring_index_skips_self_http_and_cockpit() {
        // SelfHttp and Cockpit have their own capture handles —
        // not part of BeeLogs' rings.
        assert_eq!(ring_index(LogTab::SelfHttp), None);
        assert_eq!(ring_index(LogTab::Cockpit), None);
    }

    fn mk_line(msg: &str) -> BeeLogLine {
        BeeLogLine {
            timestamp: "2026-05-16 10:00:00.000".into(),
            logger: "node/test".into(),
            message: msg.into(),
        }
    }

    #[test]
    fn beelogs_drain_routes_lines_to_correct_ring() {
        let mut bl = BeeLogs::new();
        let tx = bl.tx.clone();
        tx.send((LogTab::Errors, mk_line("e1"))).unwrap();
        tx.send((LogTab::Info, mk_line("i1"))).unwrap();
        tx.send((LogTab::Info, mk_line("i2"))).unwrap();
        // SelfHttp/Cockpit lines must be silently dropped (they have
        // their own handles, not this ring).
        tx.send((LogTab::SelfHttp, mk_line("ignored"))).unwrap();
        bl.drain();
        assert_eq!(bl.snapshot(LogTab::Errors).len(), 1);
        assert_eq!(bl.snapshot(LogTab::Info).len(), 2);
        assert_eq!(bl.snapshot(LogTab::SelfHttp).len(), 0);
        assert_eq!(bl.snapshot(LogTab::Debug).len(), 0);
    }

    #[test]
    fn beelogs_ring_evicts_oldest_at_capacity() {
        let mut bl = BeeLogs::new();
        let tx = bl.tx.clone();
        for i in 0..(RING_CAPACITY + 5) {
            tx.send((LogTab::Info, mk_line(&format!("line-{i}")))).unwrap();
        }
        bl.drain();
        let ring = bl.snapshot(LogTab::Info);
        assert_eq!(ring.len(), RING_CAPACITY);
        // First retained line should be the 6th (0..5 evicted).
        assert_eq!(ring.front().unwrap().message, "line-5");
        assert_eq!(
            ring.back().unwrap().message,
            format!("line-{}", RING_CAPACITY + 4)
        );
    }

    #[test]
    fn beelogs_respawn_clears_rings() {
        let mut bl = BeeLogs::new();
        let tx = bl.tx.clone();
        tx.send((LogTab::Errors, mk_line("stale"))).unwrap();
        bl.drain();
        assert_eq!(bl.snapshot(LogTab::Errors).len(), 1);
        bl.respawn(
            ResolvedSource::None {
                reason: "test".into(),
            },
            tokio_util::sync::CancellationToken::new(),
        );
        assert_eq!(bl.snapshot(LogTab::Errors).len(), 0);
    }

    #[test]
    fn beelogs_respawn_drops_in_flight_messages_from_old_tailer() {
        // Simulates the race where the old tailer's cancel has fired
        // but its read loop is still pushing a couple of lines into
        // the shared tx. With a fresh mpsc per respawn, those lines
        // must not appear in the new rings.
        let mut bl = BeeLogs::new();
        let old_tx = bl.tx.clone();
        bl.respawn(
            ResolvedSource::None {
                reason: "switched".into(),
            },
            tokio_util::sync::CancellationToken::new(),
        );
        // The old tailer's still-alive send-handle pushes lines
        // *after* the respawn — these go to the dropped receiver.
        let _ = old_tx.send((LogTab::Info, mk_line("stale-after-switch")));
        bl.drain();
        assert_eq!(bl.snapshot(LogTab::Info).len(), 0);
    }

    #[test]
    fn beelogs_respawn_updates_source_label_for_none() {
        // None doesn't spawn a tailer so this test stays sync-only.
        let mut bl = BeeLogs::new();
        bl.respawn(
            ResolvedSource::None {
                reason: "x".into(),
            },
            tokio_util::sync::CancellationToken::new(),
        );
        assert!(matches!(bl.source, ResolvedSource::None { .. }));
    }

    #[tokio::test]
    async fn beelogs_respawn_with_file_updates_source_label() {
        // File source spawns a tailer; needs a tokio runtime.
        let mut bl = BeeLogs::new();
        bl.respawn(
            ResolvedSource::File {
                path: PathBuf::from("/nonexistent/x.log"),
                origin: SourceOrigin::Supervisor,
            },
            tokio_util::sync::CancellationToken::new(),
        );
        match &bl.source {
            ResolvedSource::File { origin, .. } => {
                assert_eq!(*origin, SourceOrigin::Supervisor);
            }
            _ => panic!("source not updated"),
        }
    }
}
