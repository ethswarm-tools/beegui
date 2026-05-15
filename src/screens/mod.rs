//! Screen registry. Each screen module owns its widget rendering;
//! the data layer comes from [`bee_cockpit_core::views`].

use bee_cockpit_core::watch::BeeWatch;
use strum::{EnumIter, IntoEnumIterator};

pub mod health;
pub mod lottery;
pub mod stamps;
pub mod swap;
pub mod warmup;

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
}

impl Screen {
    pub fn label(self) -> &'static str {
        match self {
            Screen::Health => "Health",
            Screen::Stamps => "Stamps",
            Screen::Swap => "Swap",
            Screen::Lottery => "Lottery",
            Screen::Warmup => "Warmup",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Screen::Health => "1",
            Screen::Stamps => "2",
            Screen::Swap => "3",
            Screen::Lottery => "4",
            Screen::Warmup => "5",
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

pub fn draw(screen: Screen, ui: &mut egui::Ui, watch: &BeeWatch, state: &mut ScreenState) {
    match screen {
        Screen::Health => health::draw(ui, watch),
        Screen::Stamps => stamps::draw(ui, watch),
        Screen::Swap => swap::draw(ui, watch),
        Screen::Lottery => lottery::draw(ui, watch),
        Screen::Warmup => warmup::draw(ui, watch, &mut state.warmup),
    }
}
