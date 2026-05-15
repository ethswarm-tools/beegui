//! Alerts pipeline. Each frame the App calls [`AlertsPipeline::observe`]
//! with the current gate list. New gate transitions are:
//!   1. appended to a ring buffer (drives the in-app popup),
//!   2. fired at the configured webhook (if any),
//!   3. raised as desktop notifications (if `[notifications].desktop`).

use std::collections::VecDeque;
use std::time::SystemTime;

use bee_cockpit_core::alerts::{Alert, AlertState, fire};
use bee_cockpit_core::config::{AlertsConfig, NotificationsConfig};
use bee_cockpit_core::views::health::{Gate, GateStatus};
use tokio::runtime::Handle;

const HISTORY_CAP: usize = 100;

pub struct AlertsPipeline {
    state: AlertState,
    history: VecDeque<TimestampedAlert>,
    last_seen_unack: usize,
    alerts_cfg: AlertsConfig,
    notif_cfg: NotificationsConfig,
    rt: Option<Handle>,
}

#[derive(Clone)]
pub struct TimestampedAlert {
    pub when: SystemTime,
    pub alert: Alert,
}

impl AlertsPipeline {
    pub fn new(
        alerts_cfg: AlertsConfig,
        notif_cfg: NotificationsConfig,
        rt: Option<Handle>,
    ) -> Self {
        Self {
            state: AlertState::new(alerts_cfg.debounce_secs),
            history: VecDeque::new(),
            last_seen_unack: 0,
            alerts_cfg,
            notif_cfg,
            rt,
        }
    }

    pub fn observe(&mut self, gates: &[Gate]) {
        let alerts = self.state.diff_and_record(gates);
        for a in alerts {
            self.escalate(&a);
            if self.history.len() >= HISTORY_CAP {
                self.history.pop_front();
            }
            self.history.push_back(TimestampedAlert {
                when: SystemTime::now(),
                alert: a,
            });
        }
    }

    fn escalate(&self, a: &Alert) {
        if a.is_worth_alerting() {
            if let Some(url) = self.alerts_cfg.webhook_url.clone() {
                if let Some(rt) = &self.rt {
                    let alert = a.clone();
                    rt.spawn(async move {
                        if let Err(e) = fire(&url, &alert).await {
                            tracing::warn!(target: "beegui::alerts", "webhook fire failed: {e}");
                        }
                    });
                }
            }
            if self.notif_cfg.desktop && matches!(a.to, GateStatus::Fail | GateStatus::Warn) {
                fire_desktop_notification(a);
            }
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

    pub fn webhook_configured(&self) -> bool {
        self.alerts_cfg.webhook_url.is_some()
    }

    pub fn desktop_enabled(&self) -> bool {
        self.notif_cfg.desktop
    }
}

fn fire_desktop_notification(a: &Alert) {
    let mut nb = notify_rust::Notification::new();
    let headline = format!("beegui: {}", a.message_line());
    nb.summary(&headline);
    if let Some(why) = &a.why {
        nb.body(why);
    }
    nb.appname("beegui");
    if let Err(e) = nb.show() {
        tracing::warn!(target: "beegui::alerts", "desktop notification failed: {e}");
    }
}

