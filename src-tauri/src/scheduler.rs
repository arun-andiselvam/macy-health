//! Interval scheduler. Uses wall-clock time (`SystemTime`) rather than
//! `Instant`, which stops advancing while a Mac is asleep.

use std::{
    sync::Mutex,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{Datelike, Local, TimeZone, Timelike};
use tauri::{AppHandle, Manager};

use crate::{
    popup::{self, PopupAction, PopupReminder},
    reminder::{LocalTime, Reminder},
};

const SNOOZE_MINUTES: u64 = 5;
/// A reminder this late means the Mac was asleep (or the app suspended):
/// restart its interval instead of firing a stale reminder.
const STALE_AFTER_MINUTES: u64 = 2;

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
    /// Converts epoch ms to local weekday/time. Swappable for tests.
    local: fn(u64) -> LocalTime,
}

impl Scheduler {
    pub fn new(reminders: Vec<Reminder>, minute_ms: u64, now: u64) -> Self {
        let mut scheduler = Self {
            entries: Vec::new(),
            minute_ms,
            local: local_time,
        };
        for reminder in reminders {
            scheduler.upsert(reminder, now);
        }
        scheduler
    }

    pub fn reminders(&self) -> Vec<Reminder> {
        self.entries.iter().map(|e| e.reminder.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<&Reminder> {
        self.entries
            .iter()
            .find(|e| e.reminder.id == id)
            .map(|e| &e.reminder)
    }

    /// Adds or replaces a reminder. Its timer restarts when it's new, re-enabled,
    /// or its interval changed; other edits keep the current countdown.
    pub fn upsert(&mut self, reminder: Reminder, now: u64) {
        let next_due = now + reminder.interval_minutes * self.minute_ms;
        match self
            .entries
            .iter_mut()
            .find(|e| e.reminder.id == reminder.id)
        {
            Some(entry) => {
                let restart = entry.reminder.interval_minutes != reminder.interval_minutes
                    || (!entry.reminder.enabled && reminder.enabled);
                if restart {
                    entry.next_due = next_due;
                }
                entry.reminder = reminder;
            }
            None => self.entries.push(Entry {
                reminder,
                next_due,
                showing: false,
            }),
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.reminder.id != id);
    }

    /// Returns the reminders that are due now and marks them as showing.
    pub fn tick(&mut self, now: u64) -> Vec<PopupReminder> {
        let (minute_ms, local) = (self.minute_ms, (self.local)(now));
        let mut due = Vec::new();
        for entry in &mut self.entries {
            if !entry.reminder.enabled || entry.showing || now < entry.next_due {
                continue;
            }
            let stale = now - entry.next_due > STALE_AFTER_MINUTES * minute_ms;
            if stale || !entry.reminder.is_active_at(local) {
                // Skip silently and keep the interval rolling.
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
            return; // e.g. test and preview popups
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
        let enabled = || self.entries.iter().filter(|e| e.reminder.enabled);
        let next = enabled()
            .filter(|e| !e.showing && e.reminder.is_active_at((self.local)(e.next_due)))
            .min_by_key(|e| e.next_due);
        match next {
            Some(e) => format!(
                "Next: {} {} {}",
                e.reminder.emoji,
                e.reminder.title,
                format_remaining(e.next_due.saturating_sub(now), self.minute_ms)
            ),
            None if enabled().any(|e| e.showing) => "Reminder on screen".into(),
            None if enabled().next().is_some() => "Outside active hours".into(),
            None => "All reminders are off".into(),
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

fn local_time(epoch_ms: u64) -> LocalTime {
    let dt = Local
        .timestamp_millis_opt(epoch_ms as i64)
        .single()
        .unwrap_or_else(Local::now);
    LocalTime {
        weekday: dt.weekday().number_from_monday() as u8,
        minute: (dt.hour() * 60 + dt.minute()) as u16,
    }
}

// ---------- runtime ----------

pub type SchedulerState = Mutex<Scheduler>;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `MACY_FAST=1` (debug builds only) turns minutes into seconds for testing.
fn fast_mode() -> bool {
    cfg!(debug_assertions) && std::env::var("MACY_FAST").is_ok_and(|v| v == "1")
}

pub fn init(app: &AppHandle, reminders: Vec<Reminder>) {
    let minute_ms = if fast_mode() { 1_000 } else { 60_000 };
    app.manage::<SchedulerState>(Mutex::new(Scheduler::new(reminders, minute_ms, now_ms())));
}

pub fn start_ticking(app: &AppHandle) {
    let tick = if fast_mode() {
        Duration::from_millis(500)
    } else {
        Duration::from_secs(15)
    };
    let app = app.clone();
    thread::spawn(move || loop {
        run_tick(&app);
        thread::sleep(tick);
    });
}

fn run_tick(app: &AppHandle) {
    let now = now_ms();
    let due = app.state::<SchedulerState>().lock().unwrap().tick(now);
    for reminder in due {
        popup::enqueue(app, reminder);
    }
    refresh_status(app);
}

pub fn status_text(app: &AppHandle) -> String {
    app.state::<SchedulerState>()
        .lock()
        .unwrap()
        .status_text(now_ms())
}

pub fn refresh_status(app: &AppHandle) {
    crate::tray::set_status(app, &status_text(app));
}

pub fn handle_action(app: &AppHandle, id: &str, action: PopupAction) {
    app.state::<SchedulerState>()
        .lock()
        .unwrap()
        .on_action(id, action, now_ms());
    refresh_status(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reminder::presets;

    const MIN: u64 = 60_000;

    /// Test clock: epoch 0 is Monday 00:00, always "active" unless a test
    /// narrows the reminder's hours.
    fn test_local(ms: u64) -> LocalTime {
        let minutes = ms / MIN;
        LocalTime {
            weekday: ((minutes / 1440) % 7 + 1) as u8,
            minute: (minutes % 1440) as u16,
        }
    }

    fn all_day(mut r: Reminder) -> Reminder {
        r.active_start = "00:00".into();
        r.active_end = "00:00".into();
        r
    }

    fn scheduler(reminders: Vec<Reminder>) -> Scheduler {
        let mut s = Scheduler::new(reminders.into_iter().map(all_day).collect(), MIN, 0);
        s.local = test_local;
        s
    }

    fn one(interval_minutes: u64) -> Scheduler {
        let mut r = presets().remove(0);
        r.interval_minutes = interval_minutes;
        scheduler(vec![r])
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
    fn done_skip_dismissed_restart_full_interval() {
        for action in [PopupAction::Done, PopupAction::Skip, PopupAction::Dismissed] {
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
        let mut r = presets().remove(0);
        r.enabled = false;
        let mut s = scheduler(vec![r]);
        assert!(s.tick(45 * MIN).is_empty());
        assert_eq!(s.status_text(0), "All reminders are off");
    }

    #[test]
    fn outside_active_hours_skips_and_keeps_rolling() {
        let mut r = presets().remove(0);
        r.interval_minutes = 60;
        let mut s = Scheduler::new(vec![r], MIN, 0); // 08:00–20:00
        s.local = test_local;
        // Monday 01:00: due but outside hours → skipped, next at 02:00.
        assert!(s.tick(60 * MIN).is_empty());
        assert!(s.tick(90 * MIN).is_empty());
        assert_eq!(s.status_text(90 * MIN), "Outside active hours");
        // Keeps rolling hourly until 08:00, then fires.
        for h in 2..8 {
            assert!(s.tick(h * 60 * MIN).is_empty());
        }
        assert_eq!(s.tick(8 * 60 * MIN).len(), 1);
    }

    #[test]
    fn simultaneous_reminders_all_fire() {
        let mut s = scheduler(presets());
        for t in [20, 40, 45, 60] {
            for r in s.tick(t * MIN) {
                s.on_action(&r.id, PopupAction::Done, t * MIN);
            }
        }
        // eyes 20, water 45, stand 60; after acting on each, only eyes is due at 80.
        let due: Vec<_> = s.tick(80 * MIN).into_iter().map(|r| r.id).collect();
        assert_eq!(due, ["eyes"]);
    }

    #[test]
    fn upsert_restarts_timer_only_when_interval_changes_or_reenabled() {
        let mut s = one(10);
        let mut r = s.get("water").unwrap().clone();

        r.title = "Hydrate".into();
        s.upsert(r.clone(), 5 * MIN);
        assert_eq!(s.tick(10 * MIN).len(), 1, "title edit keeps countdown");
        s.on_action("water", PopupAction::Done, 10 * MIN);

        r.interval_minutes = 30;
        s.upsert(r.clone(), 12 * MIN);
        assert!(s.tick(20 * MIN).is_empty(), "interval change restarts");
        assert_eq!(s.tick(42 * MIN).len(), 1);
        s.on_action("water", PopupAction::Done, 42 * MIN);

        r.enabled = false;
        s.upsert(r.clone(), 50 * MIN);
        r.enabled = true;
        s.upsert(r, 100 * MIN);
        assert!(s.tick(129 * MIN).is_empty(), "re-enable restarts");
        assert_eq!(s.tick(130 * MIN).len(), 1);
    }

    #[test]
    fn remove_stops_reminder() {
        let mut s = one(10);
        s.remove("water");
        assert!(s.tick(10 * MIN).is_empty());
        assert!(s.reminders().is_empty());
    }

    #[test]
    fn status_shows_soonest_not_showing() {
        let mut s = scheduler(presets());
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
