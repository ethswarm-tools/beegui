//! Command palette. Opens with `:` or `Ctrl+P`. The user picks a
//! verb (with optional arguments); the palette runs it on the
//! tokio handle and surfaces the result as a notification banner.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use std::path::PathBuf;

use bee::swarm::Topic;
use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::feed_probe;
use bee_cockpit_core::log_capture::LogCapture;
use bee_cockpit_core::pprof_bundle;
use bee_cockpit_core::stamp_preview;
use bee_cockpit_core::uploads;
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
        name: "upload",
        summary: "upload a file or directory to Bee",
        usage: ":upload <path> [batch-prefix]",
    },
    VerbSpec {
        name: "feed-probe",
        summary: "fetch latest feed update",
        usage: ":feed-probe <owner> <topic>",
    },
    VerbSpec {
        name: "pss",
        summary: "send a PSS message",
        usage: ":pss <topic> <payload> [batch-prefix]",
    },
    VerbSpec {
        name: "batch",
        summary: "stamp-batch math (buy/topup/dilute/extend)",
        usage: ":batch buy <depth> [amount] | :batch topup|dilute|extend <id> <arg>",
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
        name: "nodes",
        summary: "open the node picker (same as Ctrl+N)",
        usage: ":nodes",
    },
    VerbSpec {
        name: "context",
        summary: "switch active node by name",
        usage: ":context <name>",
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
    Err,
}

impl BannerLevel {
    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Ok => egui::Color32::from_rgb(0x4a, 0xc0, 0x4a),
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
    OpenNodePicker,
    SwitchContext(String),
}

pub struct Palette {
    pub open: bool,
    pub input: String,
    pub selected: usize,
    pub history: VecDeque<String>,
    banner: Option<Banner>,
    out_tx: mpsc::UnboundedSender<Banner>,
    out_rx: mpsc::UnboundedReceiver<Banner>,
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
            "upload" => self.verb_upload(&args, api, rt),
            "feed-probe" | "fp" => self.verb_feed_probe(&args, api, rt),
            "pss" => self.verb_pss(&args, api, rt),
            "batch" => self.verb_batch(&args, api, rt),
            "diagnose" => self.verb_diagnose(api, rt),
            "logs" => vec![PaletteAction::ToggleLogs],
            "alerts" => vec![PaletteAction::ToggleAlerts],
            "nodes" | "node" => vec![PaletteAction::OpenNodePicker],
            "context" | "ctx" | "switch" => self.verb_context(&args),
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

    fn verb_context(&mut self, args: &[&str]) -> Vec<PaletteAction> {
        let Some(name) = args.first() else {
            self.set_banner(BannerLevel::Err, "usage: :context <name>");
            return Vec::new();
        };
        vec![PaletteAction::SwitchContext((*name).to_string())]
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

    fn verb_upload(
        &mut self,
        args: &[&str],
        api: Arc<ApiClient>,
        rt: &Handle,
    ) -> Vec<PaletteAction> {
        let Some(path_str) = args.first() else {
            self.set_banner(BannerLevel::Err, "usage: :upload <path> [batch-prefix]");
            return Vec::new();
        };
        let path = PathBuf::from(path_str);
        let prefix = args.get(1).map(|s| (*s).to_string());
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                self.set_banner(BannerLevel::Err, format!("stat {}: {e}", path.display()));
                return Vec::new();
            }
        };
        let is_dir = meta.is_dir();
        self.set_banner(
            BannerLevel::Ok,
            format!(
                "uploading {} ({})…",
                path.display(),
                if is_dir { "directory" } else { "file" }
            ),
        );
        let tx = self.out_tx.clone();
        rt.spawn(async move {
            let outcome = run_upload(api, path, is_dir, prefix).await;
            let _ = tx.send(outcome);
        });
        Vec::new()
    }

    fn verb_feed_probe(
        &mut self,
        args: &[&str],
        api: Arc<ApiClient>,
        rt: &Handle,
    ) -> Vec<PaletteAction> {
        if args.len() < 2 {
            self.set_banner(BannerLevel::Err, "usage: :feed-probe <owner> <topic>");
            return Vec::new();
        }
        let owner_s = args[0].to_string();
        let topic_s = args[1].to_string();
        let parsed = match feed_probe::parse_args(&owner_s, &topic_s) {
            Ok(p) => p,
            Err(e) => {
                self.set_banner(BannerLevel::Err, format!("feed-probe: {e}"));
                return Vec::new();
            }
        };
        self.set_banner(BannerLevel::Ok, "probing feed…");
        let tx = self.out_tx.clone();
        rt.spawn(async move {
            let outcome = match feed_probe::probe(api, parsed).await {
                Ok(r) => Banner {
                    level: BannerLevel::Ok,
                    text: format!(
                        "feed-probe: idx={} payload={}B ref={}",
                        r.index,
                        r.payload_bytes,
                        r.reference_hex.unwrap_or_else(|| "—".into()),
                    ),
                    when: Instant::now(),
                },
                Err(e) => Banner {
                    level: BannerLevel::Err,
                    text: format!("feed-probe: {e}"),
                    when: Instant::now(),
                },
            };
            let _ = tx.send(outcome);
        });
        Vec::new()
    }

    fn verb_pss(
        &mut self,
        args: &[&str],
        api: Arc<ApiClient>,
        rt: &Handle,
    ) -> Vec<PaletteAction> {
        if args.len() < 2 {
            self.set_banner(
                BannerLevel::Err,
                "usage: :pss <topic> <payload> [batch-prefix]",
            );
            return Vec::new();
        }
        let topic_arg = args[0].to_string();
        let payload = args[1].as_bytes().to_vec();
        let prefix = args.get(2).map(|s| (*s).to_string());
        let topic = match parse_topic(&topic_arg) {
            Ok(t) => t,
            Err(e) => {
                self.set_banner(BannerLevel::Err, format!("topic: {e}"));
                return Vec::new();
            }
        };
        self.set_banner(BannerLevel::Ok, format!("sending PSS to {}…", &topic_arg));
        let tx = self.out_tx.clone();
        rt.spawn(async move {
            let outcome = run_pss(api, topic, payload, prefix).await;
            let _ = tx.send(outcome);
        });
        Vec::new()
    }

    fn verb_batch(
        &mut self,
        args: &[&str],
        api: Arc<ApiClient>,
        rt: &Handle,
    ) -> Vec<PaletteAction> {
        let Some(sub) = args.first() else {
            self.set_banner(
                BannerLevel::Err,
                "usage: :batch buy|topup|dilute|extend <args…>",
            );
            return Vec::new();
        };
        self.set_banner(BannerLevel::Ok, format!("computing batch {sub}…"));
        let sub = (*sub).to_string();
        let rest: Vec<String> = args[1..].iter().map(|s| (*s).to_string()).collect();
        let tx = self.out_tx.clone();
        rt.spawn(async move {
            let outcome = run_batch(api, &sub, &rest).await;
            let _ = tx.send(outcome);
        });
        Vec::new()
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
                Ok(bundle) => Banner {
                    level: BannerLevel::Ok,
                    text: bundle.summary(),
                    when: Instant::now(),
                },
                Err(e) => Banner {
                    level: BannerLevel::Err,
                    text: format!("diagnose: {e}"),
                    when: Instant::now(),
                },
            };
            let _ = tx.send(outcome);
        });
        self.set_banner(BannerLevel::Ok, "diagnose bundle running (10s sampling)…");
        Vec::new()
    }

    /// Drain async-completed banners (e.g. `:diagnose`) and apply
    /// them. Returns an empty action list — async verbs can't queue
    /// state-mutating actions for now; they only update the banner.
    pub fn pump(&mut self) -> Vec<PaletteAction> {
        while let Ok(b) = self.out_rx.try_recv() {
            self.banner = Some(b);
        }
        Vec::new()
    }
}

async fn run_upload(
    api: Arc<ApiClient>,
    path: PathBuf,
    is_dir: bool,
    prefix: Option<String>,
) -> Banner {
    let batches = match api.bee().postage().get_postage_batches().await {
        Ok(b) => b,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("upload: /stamps failed: {e}"),
                when: Instant::now(),
            };
        }
    };
    let batch = match pick_batch(&batches, prefix.as_deref()) {
        Ok(b) => b.clone(),
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("upload: {e}"),
                when: Instant::now(),
            };
        }
    };
    if is_dir {
        return run_upload_dir(api, path, batch).await;
    }
    let data = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("upload: read {}: {e}", path.display()),
                when: Instant::now(),
            };
        }
    };
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let ct = upload_content_type(&path);
    match api
        .bee()
        .file()
        .upload_file(&batch.batch_id, data, &name, &ct, None)
        .await
    {
        Ok(res) => Banner {
            level: BannerLevel::Ok,
            text: format!(
                "uploaded {} → ref {} (batch {})",
                path.display(),
                res.reference.to_hex(),
                &batch.batch_id.to_hex()[..8],
            ),
            when: Instant::now(),
        },
        Err(e) => Banner {
            level: BannerLevel::Err,
            text: format!("upload failed: {e}"),
            when: Instant::now(),
        },
    }
}

async fn run_upload_dir(
    api: Arc<ApiClient>,
    path: PathBuf,
    batch: bee::postage::PostageBatch,
) -> Banner {
    let walked = match uploads::walk_dir(&path) {
        Ok(w) => w,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("upload: walk {}: {e}", path.display()),
                when: Instant::now(),
            };
        }
    };
    if walked.entries.is_empty() {
        return Banner {
            level: BannerLevel::Err,
            text: format!("upload: {} is empty", path.display()),
            when: Instant::now(),
        };
    }
    let opts = bee::api::CollectionUploadOptions {
        index_document: walked.default_index.clone(),
        ..Default::default()
    };
    match api
        .bee()
        .file()
        .upload_collection_entries(&batch.batch_id, &walked.entries, Some(&opts))
        .await
    {
        Ok(res) => Banner {
            level: BannerLevel::Ok,
            text: format!(
                "uploaded {} files ({} bytes) → ref {} (batch {})",
                walked.entries.len(),
                walked.total_bytes,
                res.reference.to_hex(),
                &batch.batch_id.to_hex()[..8],
            ),
            when: Instant::now(),
        },
        Err(e) => Banner {
            level: BannerLevel::Err,
            text: format!("upload failed: {e}"),
            when: Instant::now(),
        },
    }
}

fn pick_batch<'a>(
    batches: &'a [bee::postage::PostageBatch],
    prefix: Option<&str>,
) -> Result<&'a bee::postage::PostageBatch, String> {
    if let Some(p) = prefix {
        return stamp_preview::match_batch_prefix(batches, p);
    }
    let usable: Vec<&bee::postage::PostageBatch> = batches
        .iter()
        .filter(|b| b.usable && b.batch_ttl > 0)
        .collect();
    if usable.is_empty() {
        return Err("no usable batch with positive TTL — buy or topup one first".into());
    }
    usable
        .into_iter()
        .max_by_key(|b| b.batch_ttl)
        .ok_or_else(|| "no usable batch".into())
}

fn upload_content_type(path: &std::path::Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("html") | Some("htm") => "text/html",
        Some("txt") | Some("md") => "text/plain",
        Some("json") => "application/json",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("tar") => "application/x-tar",
        Some("gz") | Some("tgz") => "application/gzip",
        Some("wasm") => "application/wasm",
        _ => "",
    }
    .to_string()
}

fn parse_topic(s: &str) -> Result<Topic, String> {
    let trimmed = s.trim().trim_start_matches("0x");
    if trimmed.len() == 64
        && let Ok(bytes) = decode_hex_32(trimmed)
    {
        return Topic::new(&bytes).map_err(|e| e.to_string());
    }
    // Treat as utf-8 string and apply Bee's keccak256-of-string convention.
    Ok(Topic::from_string(s))
}

fn decode_hex_32(s: &str) -> Result<[u8; 32], String> {
    let mut arr = [0u8; 32];
    for i in 0..32 {
        arr[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|e| format!("bad hex at {i}: {e}"))?;
    }
    Ok(arr)
}

async fn run_pss(
    api: Arc<ApiClient>,
    topic: Topic,
    payload: Vec<u8>,
    prefix: Option<String>,
) -> Banner {
    let batches = match api.bee().postage().get_postage_batches().await {
        Ok(b) => b,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("pss: /stamps failed: {e}"),
                when: Instant::now(),
            };
        }
    };
    let batch = match pick_batch(&batches, prefix.as_deref()) {
        Ok(b) => b.clone(),
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("pss: {e}"),
                when: Instant::now(),
            };
        }
    };
    // Targets list — empty means broadcast.
    match api
        .bee()
        .pss()
        .send(&batch.batch_id, &topic, "", payload.clone(), None)
        .await
    {
        Ok(_) => Banner {
            level: BannerLevel::Ok,
            text: format!(
                "pss sent — topic {} · {} bytes · batch {}",
                topic.to_hex(),
                payload.len(),
                &batch.batch_id.to_hex()[..8],
            ),
            when: Instant::now(),
        },
        Err(e) => Banner {
            level: BannerLevel::Err,
            text: format!("pss send failed: {e}"),
            when: Instant::now(),
        },
    }
}

async fn run_batch(api: Arc<ApiClient>, sub: &str, rest: &[String]) -> Banner {
    match sub {
        "buy" => batch_buy(api, rest).await,
        "topup" | "dilute" | "extend" => batch_modify(api, sub, rest).await,
        other => Banner {
            level: BannerLevel::Err,
            text: format!("batch: unknown subverb {other:?}"),
            when: Instant::now(),
        },
    }
}

async fn batch_buy(api: Arc<ApiClient>, rest: &[String]) -> Banner {
    if rest.len() < 2 {
        return Banner {
            level: BannerLevel::Err,
            text: "usage: :batch buy <depth> <amount_plur>".into(),
            when: Instant::now(),
        };
    }
    let depth: u8 = match rest[0].parse() {
        Ok(d) => d,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("depth: {e}"),
                when: Instant::now(),
            };
        }
    };
    let amount: num_bigint::BigInt = match rest[1].parse() {
        Ok(v) => v,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("amount: {e}"),
                when: Instant::now(),
            };
        }
    };
    let chain = match api.bee().debug().chain_state().await {
        Ok(c) => c,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("batch: /chainstate failed: {e}"),
                when: Instant::now(),
            };
        }
    };
    match stamp_preview::buy_preview(depth, amount, &chain) {
        Ok(p) => Banner {
            level: BannerLevel::Ok,
            text: p.summary(),
            when: Instant::now(),
        },
        Err(e) => Banner {
            level: BannerLevel::Err,
            text: format!("buy-preview: {e}"),
            when: Instant::now(),
        },
    }
}

async fn batch_modify(api: Arc<ApiClient>, sub: &str, rest: &[String]) -> Banner {
    if rest.len() < 2 {
        return Banner {
            level: BannerLevel::Err,
            text: format!("usage: :batch {sub} <batch-id-prefix> <arg>"),
            when: Instant::now(),
        };
    }
    let bee = api.bee();
    let postage = bee.postage();
    let debug = bee.debug();
    let (batches, chain) = tokio::join!(postage.get_postage_batches(), debug.chain_state());
    let batches = match batches {
        Ok(b) => b,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("batch: /stamps failed: {e}"),
                when: Instant::now(),
            };
        }
    };
    let chain = match chain {
        Ok(c) => c,
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("batch: /chainstate failed: {e}"),
                when: Instant::now(),
            };
        }
    };
    let batch = match stamp_preview::match_batch_prefix(&batches, &rest[0]) {
        Ok(b) => b.clone(),
        Err(e) => {
            return Banner {
                level: BannerLevel::Err,
                text: format!("batch: {e}"),
                when: Instant::now(),
            };
        }
    };
    let arg = &rest[1];
    let result: Result<String, String> = match sub {
        "topup" => arg
            .parse::<num_bigint::BigInt>()
            .map_err(|e| format!("topup amount: {e}"))
            .and_then(|amount| {
                stamp_preview::topup_preview(&batch, amount, &chain).map(|p| p.summary())
            }),
        "dilute" => arg
            .parse::<u8>()
            .map_err(|e| format!("dilute depth: {e}"))
            .and_then(|nd| stamp_preview::dilute_preview(&batch, nd).map(|p| p.summary())),
        "extend" => arg
            .parse::<i64>()
            .map_err(|e| format!("extend seconds: {e}"))
            .and_then(|secs| {
                stamp_preview::extend_preview(&batch, secs, &chain).map(|p| p.summary())
            }),
        _ => unreachable!(),
    };
    match result {
        Ok(text) => Banner {
            level: BannerLevel::Ok,
            text,
            when: Instant::now(),
        },
        Err(text) => Banner {
            level: BannerLevel::Err,
            text,
            when: Instant::now(),
        },
    }
}

fn chrono_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
