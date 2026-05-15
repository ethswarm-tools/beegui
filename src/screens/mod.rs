//! Screen registry. Each screen module owns its widget rendering;
//! the data layer comes from [`bee_cockpit_core::views`].

use bee_cockpit_core::watch::BeeWatch;
use strum::{EnumIter, IntoEnumIterator};

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
    url: &str,
) {
    match screen {
        Screen::Health => health::draw(ui, watch),
        Screen::Stamps => stamps::draw(ui, watch),
        Screen::Swap => swap::draw(ui, watch),
        Screen::Lottery => lottery::draw(ui, watch),
        Screen::Warmup => warmup::draw(ui, watch, &mut state.warmup),
        Screen::Peers => peers::draw(ui, watch),
        Screen::Network => network::draw(ui, watch),
        Screen::ApiHealth => api_health::draw(ui, watch, url),
        Screen::Tags => tags::draw(ui, watch),
        Screen::Pins => pins::draw(ui, watch),
        Screen::Manifest => manifest::draw(ui),
        Screen::Watchlist => watchlist::draw(ui),
        Screen::FeedTimeline => feed_timeline::draw(ui),
        Screen::Pubsub => pubsub::draw(ui),
        Screen::Fleet => fleet::draw(ui),
    }
}
