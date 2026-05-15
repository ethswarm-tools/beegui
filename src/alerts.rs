//! Lightweight alerts pipeline: every frame we run the gate list
//! through [`bee_cockpit_core::alerts::AlertState::diff_and_record`]
//! and append surfaced transitions to a ring buffer the renderer
//! paints in a popup.

use std::collections::VecDeque;
use std::time::SystemTime;

use bee_cockpit_core::alerts::{Alert, AlertState};
use bee_cockpit_core::views::health::Gate;

const HISTORY_CAP: usize = 100;

pub struct AlertsPipeline {
    state: AlertState,
    history: VecDeque<TimestampedAlert>,
    last_seen_unack: usize,
}

#[derive(Clone)]
pub struct TimestampedAlert {
    pub when: SystemTime,
    pub alert: Alert,
}

impl AlertsPipeline {
    pub fn new(debounce_secs: u64) -> Self {
        Self {
            state: AlertState::new(debounce_secs),
            history: VecDeque::new(),
            last_seen_unack: 0,
        }
    }

    pub fn observe(&mut self, gates: &[Gate]) {
        let alerts = self.state.diff_and_record(gates);
        for a in alerts {
            if self.history.len() >= HISTORY_CAP {
                self.history.pop_front();
            }
            self.history.push_back(TimestampedAlert {
                when: SystemTime::now(),
                alert: a,
            });
        }
    }

    pub fn history(&self) -> impl DoubleEndedIterator<Item = &TimestampedAlert> {
        self.history.iter()
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn unread_count(&self) -> usize {
        self.history.len().saturating_sub(self.last_seen_unack)
    }

    pub fn mark_read(&mut self) {
        self.last_seen_unack = self.history.len();
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.last_seen_unack = 0;
    }
}
