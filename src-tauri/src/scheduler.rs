//! Interval scheduler. Uses wall-clock time (`SystemTime`) rather than
//! `Instant`, which stops advancing while a Mac is asleep.

use std::{
    sync::Mutex,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager};

use crate::popup::{self, PopupAction, PopupReminder};

const SNOOZE_MINUTES: u64 = 5;
/// A reminder this late means the Mac was asleep (or the app suspended):
/// restart its interval instead of firing a stale reminder.
const STALE_AFTER_MINUTES: u64 = 2;

#[derive(Clone, Debug)]
pub struct Reminder {
    pub id: String,
    pub kind: String,
    pub emoji: String,
    pub title: String,
    pub message: String,
    pub interval_minutes: u64,
    pub enabled: bool,
}

impl Reminder {
    fn to_popup(&self) -> PopupReminder {
        PopupReminder {
            id: self.id.clone(),
            kind: self.kind.clone(),
            emoji: self.emoji.clone(),
            title: self.title.clone(),
            message: self.message.clone(),
        }
    }
}

/// Hard-coded until Settings (Phase 5).
pub fn default_reminders() -> Vec<Reminder> {
    let preset = |id: &str, emoji: &str, title: &str, message: &str, interval_minutes| Reminder {
        id: id.into(),
        kind: id.into(),
        emoji: emoji.into(),
        title: title.into(),
        message: message.into(),
        interval_minutes,
        enabled: true,
    };
    vec![
        preset(
            "water",
            "💧",
            "Drink water",
            "Time for a glass of water. Stay hydrated!",
            45,
        ),
        preset(
            "eyes",
            "👀",
            "Rest your eyes",
            "Look at something 20 feet away for 20 seconds.",
            20,
        ),
        preset(
            "stand",
            "🧍",
            "Stand up",
            "Stand, stretch and walk around for a minute.",
            60,
        ),
    ]
}

#[derive(Debug)]
struct Entry {
    reminder: Reminder,
    next_due: u64,
    /// Fired and waiting for a popup action; not rescheduled until then.
    showing: bool,
}

pub struct Scheduler {
    entries: Vec<Entry>,
    /// Length of a "minute" in ms. 60_000 normally, 1_000 in fast dev mode.
    minute_ms: u64,
}

impl Scheduler {
    pub fn new(reminders: Vec<Reminder>, minute_ms: u64, now: u64) -> Self {
        let entries = reminders
            .into_iter()
            .map(|reminder| Entry {
                next_due: now + reminder.interval_minutes * minute_ms,
                reminder,
                showing: false,
            })
            .collect();
        Self { entries, minute_ms }
    }

    /// Returns the reminders that are due now and marks them as showing.
    pub fn tick(&mut self, now: u64) -> Vec<PopupReminder> {
        let minute_ms = self.minute_ms;
        let mut due = Vec::new();
        for entry in &mut self.entries {
            if !entry.reminder.enabled || entry.showing || now < entry.next_due {
                continue;
            }
            if now - entry.next_due > STALE_AFTER_MINUTES * minute_ms {
                entry.next_due = now + entry.reminder.interval_minutes * minute_ms;
                continue;
            }
            entry.showing = true;
            due.push(entry.reminder.to_popup());
        }
        due
    }

    pub fn on_action(&mut self, id: &str, action: PopupAction, now: u64) {
        let Some(entry) = self.entries.iter_mut().find(|e| e.reminder.id == id) else {
            return; // e.g. test popups
        };
        let delay_minutes = match action {
            PopupAction::Snooze => SNOOZE_MINUTES,
            PopupAction::Done | PopupAction::Skip | PopupAction::Dismissed => {
                entry.reminder.interval_minutes
            }
        };
        entry.showing = false;
        entry.next_due = now + delay_minutes * self.minute_ms;
    }

    pub fn status_text(&self, now: u64) -> String {
        let next = self
            .entries
            .iter()
            .filter(|e| e.reminder.enabled && !e.showing)
            .min_by_key(|e| e.next_due);
        match next {
            Some(e) => format!(
                "Next: {} {} {}",
                e.reminder.emoji,
                e.reminder.title,
                format_remaining(e.next_due.saturating_sub(now), self.minute_ms)
            ),
            None if self.entries.iter().any(|e| e.showing) => "Reminder on screen".into(),
            None => "No reminders scheduled".into(),
        }
    }
}

fn format_remaining(ms: u64, minute_ms: u64) -> String {
    let minutes = ms.div_ceil(minute_ms);
    match (minutes / 60, minutes % 60) {
        (0, 0) => "now".into(),
        (0, m) => format!("in {m}m"),
        (h, 0) => format!("in {h}h"),
        (h, m) => format!("in {h}h {m}m"),
    }
}

// ---------- runtime ----------

pub type SchedulerState = Mutex<Scheduler>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `MACY_FAST=1` (debug builds only) turns minutes into seconds for testing.
fn fast_mode() -> bool {
    cfg!(debug_assertions) && std::env::var("MACY_FAST").is_ok_and(|v| v == "1")
}

pub fn start(app: &AppHandle) {
    let fast = fast_mode();
    let minute_ms = if fast { 1_000 } else { 60_000 };
    let tick = if fast {
        Duration::from_millis(500)
    } else {
        Duration::from_secs(15)
    };

    app.manage::<SchedulerState>(Mutex::new(Scheduler::new(
        default_reminders(),
        minute_ms,
        now_ms(),
    )));

    let app = app.clone();
    thread::spawn(move || loop {
        run_tick(&app);
        thread::sleep(tick);
    });
}

fn run_tick(app: &AppHandle) {
    let now = now_ms();
    let (due, status) = {
        let state = app.state::<SchedulerState>();
        let mut scheduler = state.lock().unwrap();
        (scheduler.tick(now), scheduler.status_text(now))
    };
    for reminder in due {
        popup::enqueue(app, reminder);
    }
    crate::tray::set_status(app, &status);
}

pub fn handle_action(app: &AppHandle, id: &str, action: PopupAction) {
    let now = now_ms();
    let status = {
        let state = app.state::<SchedulerState>();
        let mut scheduler = state.lock().unwrap();
        scheduler.on_action(id, action, now);
        scheduler.status_text(now)
    };
    crate::tray::set_status(app, &status);
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    fn one(interval_minutes: u64) -> Scheduler {
        let mut r = default_reminders().remove(0);
        r.interval_minutes = interval_minutes;
        Scheduler::new(vec![r], MIN, 0)
    }

    #[test]
    fn fires_only_when_due() {
        let mut s = one(10);
        assert!(s.tick(10 * MIN - 1).is_empty());
        assert_eq!(s.tick(10 * MIN).len(), 1);
    }

    #[test]
    fn does_not_refire_while_showing() {
        let mut s = one(10);
        assert_eq!(s.tick(10 * MIN).len(), 1);
        assert!(s.tick(11 * MIN).is_empty());
        assert!(s.tick(100 * MIN).is_empty());
    }

    #[test]
    fn done_restarts_full_interval() {
        let mut s = one(10);
        s.tick(10 * MIN);
        s.on_action("water", PopupAction::Done, 10 * MIN);
        assert!(s.tick(20 * MIN - 1).is_empty());
        assert_eq!(s.tick(20 * MIN).len(), 1);
    }

    #[test]
    fn dismissed_and_skip_restart_full_interval() {
        for action in [PopupAction::Dismissed, PopupAction::Skip] {
            let mut s = one(10);
            s.tick(10 * MIN);
            s.on_action("water", action, 10 * MIN);
            assert!(s.tick(20 * MIN - 1).is_empty());
            assert_eq!(s.tick(20 * MIN).len(), 1);
        }
    }

    #[test]
    fn snooze_returns_in_five_minutes() {
        let mut s = one(45);
        s.tick(45 * MIN);
        s.on_action("water", PopupAction::Snooze, 45 * MIN);
        assert!(s.tick(50 * MIN - 1).is_empty());
        assert_eq!(s.tick(50 * MIN).len(), 1);
    }

    #[test]
    fn stale_after_sleep_restarts_without_firing() {
        let mut s = one(10);
        // Woke 3h later: skip the stale reminder, restart interval from now.
        assert!(s.tick(190 * MIN).is_empty());
        assert!(s.tick(200 * MIN - 1).is_empty());
        assert_eq!(s.tick(200 * MIN).len(), 1);
    }

    #[test]
    fn slightly_late_still_fires() {
        let mut s = one(10);
        assert_eq!(s.tick(11 * MIN).len(), 1);
    }

    #[test]
    fn disabled_never_fires() {
        let mut r = default_reminders().remove(0);
        r.enabled = false;
        let mut s = Scheduler::new(vec![r], MIN, 0);
        assert!(s.tick(45 * MIN).is_empty());
        assert_eq!(s.status_text(0), "No reminders scheduled");
    }

    #[test]
    fn simultaneous_reminders_all_fire() {
        let mut s = Scheduler::new(default_reminders(), MIN, 0);
        // eyes 20, water 45, stand 60; after acting on each, only eyes is due at 80.
        for t in [20, 40, 45, 60] {
            for r in s.tick(t * MIN) {
                s.on_action(&r.id, PopupAction::Done, t * MIN);
            }
        }
        let due: Vec<_> = s.tick(80 * MIN).into_iter().map(|r| r.id).collect();
        assert_eq!(due, ["eyes"]);
    }

    #[test]
    fn status_shows_soonest_not_showing() {
        let mut s = Scheduler::new(default_reminders(), MIN, 0);
        assert_eq!(s.status_text(8 * MIN), "Next: 👀 Rest your eyes in 12m");
        s.tick(20 * MIN);
        assert_eq!(s.status_text(20 * MIN), "Next: 💧 Drink water in 25m");
    }

    #[test]
    fn status_when_everything_is_showing() {
        let mut s = one(10);
        s.tick(10 * MIN);
        assert_eq!(s.status_text(10 * MIN), "Reminder on screen");
    }

    #[test]
    fn formats_remaining() {
        assert_eq!(format_remaining(0, MIN), "now");
        assert_eq!(format_remaining(1, MIN), "in 1m");
        assert_eq!(format_remaining(59 * MIN, MIN), "in 59m");
        assert_eq!(format_remaining(60 * MIN, MIN), "in 1h");
        assert_eq!(format_remaining(95 * MIN, MIN), "in 1h 35m");
    }
}
