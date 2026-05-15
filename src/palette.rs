//! Command palette. Opens with `:` or `Ctrl+P`. The user picks a
//! verb (with optional arguments); the palette runs it on the
//! tokio handle and surfaces the result as a notification banner.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use std::path::PathBuf;

use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::log_capture::LogCapture;
use bee_cockpit_core::pprof_bundle;
use bee_cockpit_core::utility_verbs;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::screens::Screen;

const HISTORY_CAP: usize = 20;
const BANNER_TTL_SECS: u64 = 8;

/// One catalogued verb. `name` is what the user types; `summary`
/// shows up in the suggestion list.
#[derive(Debug, Clone, Copy)]
pub struct VerbSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub usage: &'static str,
}

/// The verbs the palette knows about. Kept short and operationally
/// useful — bee-tui exposes a longer list but most of those need an
/// interactive component bar that we don't have a GUI analogue for
/// yet.
pub const VERBS: &[VerbSpec] = &[
    VerbSpec {
        name: "go",
        summary: "switch screen — go <name>",
        usage: ":go health | stamps | swap | lottery | warmup | peers | network | api | tags | pins | manifest | watchlist | feed | pubsub | fleet",
    },
    VerbSpec {
        name: "hash",
        summary: "compute swarm hash of a local file",
        usage: ":hash <path>",
    },
    VerbSpec {
        name: "cid",
        summary: "compute CID for a reference",
        usage: ":cid <ref> [manifest|feed]",
    },
    VerbSpec {
        name: "inspect",
        summary: "load reference into Manifest screen",
        usage: ":inspect <ref>",
    },
    VerbSpec {
        name: "manifest",
        summary: "alias for :inspect",
        usage: ":manifest <ref>",
    },
    VerbSpec {
        name: "feed-timeline",
        summary: "load feed into Feed Timeline screen",
        usage: ":feed-timeline <owner> <topic> [max]",
    },
    VerbSpec {
        name: "durability",
        summary: "add reference to Watchlist + check",
        usage: ":durability <ref>",
    },
    VerbSpec {
        name: "diagnose",
        summary: "write a diagnose bundle to disk",
        usage: ":diagnose",
    },
    VerbSpec {
        name: "logs",
        summary: "toggle the bee::http log pane",
        usage: ":logs",
    },
    VerbSpec {
        name: "alerts",
        summary: "toggle the alerts panel",
        usage: ":alerts",
    },
    VerbSpec {
        name: "help",
        summary: "show keys + verb list",
        usage: ":help",
    },
    VerbSpec {
        name: "quit",
        summary: "exit beegui",
        usage: ":quit",
    },
];

/// Outcome of executing a verb, surfaced as a transient banner.
#[derive(Clone)]
pub struct Banner {
    pub level: BannerLevel,
    pub text: String,
    pub when: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BannerLevel {
    Ok,
    Warn,
    Err,
}

impl BannerLevel {
    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Ok => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
            Self::Warn => egui::Color32::from_rgb(0xe0, 0xb0, 0x30),
            Self::Err => egui::Color32::from_rgb(0xd0, 0x4a, 0x4a),
        }
    }
}

/// One requested action to be applied by App after `step()`. Verbs
/// run async on tokio; the side-effects that touch egui state come
/// back through this channel.
#[derive(Clone)]
pub enum PaletteAction {
    SwitchScreen(Screen),
    ToggleLogs,
    ToggleAlerts,
    ShowHelp,
    Quit,
    LoadManifest(String),
    LoadFeedTimeline {
        owner: String,
        topic: String,
        max: Option<u64>,
    },
    WatchlistAdd(String),
}

pub struct Palette {
    pub open: bool,
    pub input: String,
    pub selected: usize,
    pub history: VecDeque<String>,
    banner: Option<Banner>,
    out_tx: mpsc::UnboundedSender<PaletteOutcome>,
    out_rx: mpsc::UnboundedReceiver<PaletteOutcome>,
}

enum PaletteOutcome {
    Banner(Banner),
    Action(PaletteAction),
}

impl Default for Palette {
    fn default() -> Self {
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        Self {
            open: false,
            input: String::new(),
            selected: 0,
            history: VecDeque::new(),
            banner: None,
            out_tx,
            out_rx,
        }
    }
}

impl Palette {
    pub fn open(&mut self) {
        self.open = true;
        self.input.clear();
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.input.clear();
    }

    pub fn banner(&self) -> Option<&Banner> {
        let b = self.banner.as_ref()?;
        if b.when.elapsed().as_secs() > BANNER_TTL_SECS {
            None
        } else {
            Some(b)
        }
    }

    pub fn set_banner(&mut self, level: BannerLevel, text: impl Into<String>) {
        self.banner = Some(Banner {
            level,
            text: text.into(),
            when: Instant::now(),
        });
    }

    pub fn suggestions(&self) -> Vec<&'static VerbSpec> {
        let trimmed = self.input.trim_start_matches(':').to_ascii_lowercase();
        let needle = trimmed.split_whitespace().next().unwrap_or("");
        if needle.is_empty() {
            return VERBS.iter().collect();
        }
        VERBS
            .iter()
            .filter(|v| v.name.contains(needle))
            .collect()
    }

    pub fn select_prev(&mut self) {
        let len = self.suggestions().len().max(1);
        self.selected = (self.selected + len - 1) % len;
    }

    pub fn select_next(&mut self) {
        let len = self.suggestions().len().max(1);
        self.selected = (self.selected + 1) % len;
    }

    /// Commit either the highlighted suggestion (if the user typed
    /// just the verb prefix) or the literal input (if they typed
    /// arguments after the verb name).
    pub fn submit(&mut self, api: Arc<ApiClient>, rt: &Handle, _log_capture: &LogCapture) -> Vec<PaletteAction> {
        let raw = self.input.trim().trim_start_matches(':').to_string();
        let line = if raw.contains(' ') || raw.is_empty() {
            // Args supplied or empty input → use the highlighted suggestion
            // verbatim only when input is empty; otherwise honour what was typed.
            if raw.is_empty() {
                let sug = self.suggestions();
                let picked = sug.get(self.selected).copied();
                match picked {
                    Some(v) => v.name.to_string(),
                    None => return Vec::new(),
                }
            } else {
                raw.clone()
            }
        } else {
            // Bare verb name typed (no args). Use it as-is so prefix-typed
            // shortcuts still work even if a longer verb sorts first.
            raw.clone()
        };
        self.push_history(&line);
        self.close();
        self.dispatch(&line, api, rt)
    }

    fn push_history(&mut self, line: &str) {
        if let Some(front) = self.history.front() {
            if front == line {
                return;
            }
        }
        if self.history.len() >= HISTORY_CAP {
            self.history.pop_back();
        }
        self.history.push_front(line.to_string());
    }

    fn dispatch(
        &mut self,
        line: &str,
        api: Arc<ApiClient>,
        rt: &Handle,
    ) -> Vec<PaletteAction> {
        let mut parts = line.split_whitespace();
        let Some(verb) = parts.next() else {
            return Vec::new();
        };
        let args: Vec<&str> = parts.collect();
        match verb {
            "go" | "g" => self.verb_go(&args),
            "health" | "stamps" | "swap" | "lottery" | "warmup" | "peers" | "network"
            | "api" | "tags" | "pins" | "manifest-screen" | "watchlist" | "feed" | "pubsub"
            | "fleet" => self.verb_go(&[verb]),
            "hash" => self.verb_hash(&args),
            "cid" => self.verb_cid(&args),
            "inspect" | "manifest" => self.verb_load_manifest(&args),
            "feed-timeline" | "ft" => self.verb_feed_timeline(&args),
            "durability" | "durability-check" => self.verb_durability(&args),
            "diagnose" => self.verb_diagnose(api, rt),
            "logs" => vec![PaletteAction::ToggleLogs],
            "alerts" => vec![PaletteAction::ToggleAlerts],
            "help" | "?" => vec![PaletteAction::ShowHelp],
            "quit" | "q" | "exit" => vec![PaletteAction::Quit],
            other => {
                self.set_banner(BannerLevel::Err, format!("unknown verb {other:?}"));
                Vec::new()
            }
        }
    }

    fn verb_go(&mut self, args: &[&str]) -> Vec<PaletteAction> {
        let Some(name) = args.first() else {
            self.set_banner(BannerLevel::Err, "usage: :go <screen>");
            return Vec::new();
        };
        let screen = match *name {
            "health" => Screen::Health,
            "stamps" => Screen::Stamps,
            "swap" => Screen::Swap,
            "lottery" => Screen::Lottery,
            "warmup" => Screen::Warmup,
            "peers" => Screen::Peers,
            "network" => Screen::Network,
            "api" | "api-health" => Screen::ApiHealth,
            "tags" => Screen::Tags,
            "pins" => Screen::Pins,
            "manifest" | "manifest-screen" => Screen::Manifest,
            "watchlist" => Screen::Watchlist,
            "feed" | "feed-timeline" => Screen::FeedTimeline,
            "pubsub" => Screen::Pubsub,
            "fleet" => Screen::Fleet,
            other => {
                self.set_banner(BannerLevel::Err, format!("unknown screen {other:?}"));
                return Vec::new();
            }
        };
        vec![PaletteAction::SwitchScreen(screen)]
    }

    fn verb_hash(&mut self, args: &[&str]) -> Vec<PaletteAction> {
        let Some(path) = args.first() else {
            self.set_banner(BannerLevel::Err, "usage: :hash <path>");
            return Vec::new();
        };
        match utility_verbs::hash_path(path) {
            Ok(r) => self.set_banner(BannerLevel::Ok, format!("hash: {r}")),
            Err(e) => self.set_banner(BannerLevel::Err, format!("hash: {e}")),
        }
        Vec::new()
    }

    fn verb_cid(&mut self, args: &[&str]) -> Vec<PaletteAction> {
        let Some(r) = args.first() else {
            self.set_banner(
                BannerLevel::Err,
                "usage: :cid <ref> [manifest|feed]",
            );
            return Vec::new();
        };
        let kind_arg = args.get(1).copied();
        let kind = match utility_verbs::parse_cid_kind(kind_arg) {
            Ok(k) => k,
            Err(e) => {
                self.set_banner(BannerLevel::Err, e);
                return Vec::new();
            }
        };
        match utility_verbs::cid_for_ref(r, kind) {
            Ok(cid) => self.set_banner(BannerLevel::Ok, format!("cid: {cid}")),
            Err(e) => self.set_banner(BannerLevel::Err, format!("cid: {e}")),
        }
        Vec::new()
    }

    fn verb_load_manifest(&mut self, args: &[&str]) -> Vec<PaletteAction> {
        let Some(r) = args.first() else {
            self.set_banner(BannerLevel::Err, "usage: :inspect <ref>");
            return Vec::new();
        };
        self.set_banner(BannerLevel::Ok, format!("loading manifest {r}"));
        vec![
            PaletteAction::SwitchScreen(Screen::Manifest),
            PaletteAction::LoadManifest((*r).to_string()),
        ]
    }

    fn verb_feed_timeline(&mut self, args: &[&str]) -> Vec<PaletteAction> {
        if args.len() < 2 {
            self.set_banner(
                BannerLevel::Err,
                "usage: :feed-timeline <owner> <topic> [max]",
            );
            return Vec::new();
        }
        let max = args.get(2).and_then(|s| s.parse::<u64>().ok());
        self.set_banner(BannerLevel::Ok, format!("loading feed {} / {}", args[0], args[1]));
        vec![
            PaletteAction::SwitchScreen(Screen::FeedTimeline),
            PaletteAction::LoadFeedTimeline {
                owner: args[0].into(),
                topic: args[1].into(),
                max,
            },
        ]
    }

    fn verb_durability(&mut self, args: &[&str]) -> Vec<PaletteAction> {
        let Some(r) = args.first() else {
            self.set_banner(BannerLevel::Err, "usage: :durability <ref>");
            return Vec::new();
        };
        self.set_banner(BannerLevel::Ok, format!("checking durability of {r}"));
        vec![
            PaletteAction::SwitchScreen(Screen::Watchlist),
            PaletteAction::WatchlistAdd((*r).to_string()),
        ]
    }

    fn verb_diagnose(&mut self, api: Arc<ApiClient>, rt: &Handle) -> Vec<PaletteAction> {
        let tx = self.out_tx.clone();
        let base = api.url.clone();
        let auth = if api.authenticated {
            std::env::var("BEE_NODE_TOKEN").ok()
        } else {
            None
        };
        let dir = PathBuf::from(format!("/tmp/beegui-diagnose-{}", chrono_id()));
        rt.spawn(async move {
            let outcome = match pprof_bundle::fetch_and_write(&base, auth, 10, dir).await {
                Ok(bundle) => PaletteOutcome::Banner(Banner {
                    level: BannerLevel::Ok,
                    text: bundle.summary(),
                    when: Instant::now(),
                }),
                Err(e) => PaletteOutcome::Banner(Banner {
                    level: BannerLevel::Err,
                    text: format!("diagnose: {e}"),
                    when: Instant::now(),
                }),
            };
            let _ = tx.send(outcome);
        });
        self.set_banner(BannerLevel::Ok, "diagnose bundle running (10s sampling)…");
        Vec::new()
    }

    /// Drain async-completed outcomes (e.g. `:diagnose`) and merge
    /// them into the banner / action stream. Called once per frame.
    pub fn pump(&mut self) -> Vec<PaletteAction> {
        let mut out = Vec::new();
        while let Ok(o) = self.out_rx.try_recv() {
            match o {
                PaletteOutcome::Banner(b) => self.banner = Some(b),
                PaletteOutcome::Action(a) => out.push(a),
            }
        }
        out
    }
}

fn chrono_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
