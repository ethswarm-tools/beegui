//! Screen registry. Each screen module owns its widget rendering;
//! the data layer comes from [`bee_cockpit_core::views`].

use std::sync::Arc;

use bee_cockpit_core::api::ApiClient;
use bee_cockpit_core::fleet::FleetSnapshot;
use bee_cockpit_core::log_capture::LogCapture;
use bee_cockpit_core::watch::BeeWatch;
use strum::{EnumIter, IntoEnumIterator};
use tokio::runtime::Handle;
use tokio::sync::watch;

pub struct DrawContext<'a> {
    pub url: &'a str,
    pub active_name: &'a str,
    pub api: Arc<ApiClient>,
    pub rt: Handle,
    pub fleet_rx: Option<&'a watch::Receiver<FleetSnapshot>>,
    pub fleet_resync: Option<&'a tokio::sync::mpsc::UnboundedSender<()>>,
    pub log_capture: &'a LogCapture,
}

pub mod api_health;
pub mod feed_timeline;
pub mod fleet;
pub mod health;
pub mod lottery;
pub mod manifest;
pub mod network;
pub mod peers;
pub mod pins;
pub mod pubsub;
pub mod stamps;
pub mod swap;
pub mod tags;
pub mod warmup;
pub mod watchlist;

#[derive(Default)]
pub struct ScreenState {
    pub warmup: warmup::WarmupState,
    pub manifest: manifest::ManifestState,
    pub feed_timeline: feed_timeline::FeedTimelineState,
    pub watchlist: watchlist::WatchlistState,
    pub pubsub: pubsub::PubsubState,
    pub peers: peers::PeersScreenState,
    pub stamps: stamps::StampsScreenState,
    pub tags: tags::TagsScreenState,
    pub pins: pins::PinsScreenState,
    pub fleet: fleet::FleetScreenState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum Screen {
    Health,
    Stamps,
    Swap,
    Lottery,
    Warmup,
    Peers,
    Network,
    ApiHealth,
    Tags,
    Pins,
    Manifest,
    Watchlist,
    FeedTimeline,
    Pubsub,
    Fleet,
}

impl Screen {
    pub fn label(self) -> &'static str {
        match self {
            Screen::Health => "Health",
            Screen::Stamps => "Stamps",
            Screen::Swap => "Swap",
            Screen::Lottery => "Lottery",
            Screen::Warmup => "Warmup",
            Screen::Peers => "Peers",
            Screen::Network => "Network",
            Screen::ApiHealth => "API",
            Screen::Tags => "Tags",
            Screen::Pins => "Pins",
            Screen::Manifest => "Manifest",
            Screen::Watchlist => "Watchlist",
            Screen::FeedTimeline => "Feed",
            Screen::Pubsub => "Pubsub",
            Screen::Fleet => "Fleet",
        }
    }

    pub fn all() -> Vec<Screen> {
        Screen::iter().collect()
    }

    pub fn from_index(i: usize) -> Option<Screen> {
        Screen::iter().nth(i)
    }

    pub fn index(self) -> usize {
        Screen::iter().position(|s| s == self).unwrap_or(0)
    }
}

pub fn draw(
    screen: Screen,
    ui: &mut egui::Ui,
    watch: &BeeWatch,
    state: &mut ScreenState,
    ctx: DrawContext<'_>,
) {
    match screen {
        Screen::Health => health::draw(ui, watch),
        Screen::Stamps => stamps::draw(ui, watch, &mut state.stamps, ctx.api.clone(), &ctx.rt),
        Screen::Swap => swap::draw(ui, watch),
        Screen::Lottery => lottery::draw(ui, watch),
        Screen::Warmup => warmup::draw(ui, watch, &mut state.warmup),
        Screen::Peers => peers::draw(ui, watch, &mut state.peers, ctx.api.clone(), &ctx.rt),
        Screen::Network => network::draw(ui, watch),
        Screen::ApiHealth => api_health::draw(ui, watch, ctx.url, ctx.log_capture),
        Screen::Tags => tags::draw(ui, watch, &mut state.tags),
        Screen::Pins => pins::draw(ui, watch, &mut state.pins, ctx.api.clone(), &ctx.rt),
        Screen::Manifest => manifest::draw(ui, &mut state.manifest, ctx.api.clone(), &ctx.rt),
        Screen::Watchlist => {
            watchlist::draw(ui, &mut state.watchlist, ctx.api.clone(), &ctx.rt)
        }
        Screen::FeedTimeline => {
            feed_timeline::draw(ui, &mut state.feed_timeline, ctx.api.clone(), &ctx.rt)
        }
        Screen::Pubsub => pubsub::draw(ui, &mut state.pubsub, ctx.api.clone(), &ctx.rt),
        Screen::Fleet => fleet::draw(
            ui,
            ctx.fleet_rx,
            ctx.fleet_resync,
            ctx.active_name,
            &mut state.fleet,
        ),
    }
}
