//! Interval scheduler. Uses wall-clock time (`SystemTime`) rather than
//! `Instant`, which stops advancing while a Mac is asleep.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
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

/// One reminder's line in the tray menu.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuRow {
    pub id: String,
    pub name: String,
    pub time_left: Option<String>,
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
    /// Converts epoch ms to local weekday/time. Swappable for tests.
    local: fn(u64) -> LocalTime,
    /// All reminders paused until this time (epoch ms).
    paused_until: Option<u64>,
    /// Since when the user has been away (epoch ms); nothing fires meanwhile.
    away_since: Option<u64>,
}

impl Scheduler {
    pub fn new(reminders: Vec<Reminder>, minute_ms: u64, now: u64) -> Self {
        let mut scheduler = Self {
            entries: Vec::new(),
            minute_ms,
            local: local_time,
            paused_until: None,
            away_since: None,
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

    pub fn paused_until(&self) -> Option<u64> {
        self.paused_until
    }

    pub fn pause(&mut self, until: u64) {
        self.paused_until = Some(until);
    }

    /// Ends a pause; every reminder starts a fresh interval.
    pub fn resume(&mut self, now: u64) {
        self.paused_until = None;
        self.restart_all(now);
    }

    /// Resumes if the pause has run out. Returns true when it did.
    pub fn expire_pause(&mut self, now: u64) -> bool {
        match self.paused_until {
            Some(until) if now >= until => {
                self.resume(now);
                true
            }
            _ => false,
        }
    }

    pub fn is_away(&self) -> bool {
        self.away_since.is_some()
    }

    /// Records whether the user is away. Returns true when the state changed.
    pub fn set_away(&mut self, away: bool, now: u64) -> bool {
        match (away, self.away_since) {
            (true, None) => self.away_since = Some(now),
            (false, Some(since)) => {
                self.away_since = None;
                self.return_from_break(since, now);
            }
            _ => return false,
        }
        true
    }

    /// After time away (idle, locked, or the Mac asleep): eye and stand-up
    /// reminders start a fresh interval; the rest resume where they paused.
    /// Popups closed while away come back if they're still due.
    pub fn return_from_break(&mut self, since: u64, now: u64) {
        let away_for = now.saturating_sub(since);
        for entry in &mut self.entries {
            entry.showing = false;
            if entry.reminder.restarts_after_break() {
                entry.next_due = now + entry.reminder.interval_minutes * self.minute_ms;
            } else {
                entry.next_due = entry.next_due.saturating_add(away_for).max(now);
            }
        }
    }

    fn restart_all(&mut self, now: u64) {
        for entry in &mut self.entries {
            entry.showing = false;
            entry.next_due = now + entry.reminder.interval_minutes * self.minute_ms;
        }
    }

    /// Returns the reminders that are due now and marks them as showing.
    pub fn tick(&mut self, now: u64) -> Vec<PopupReminder> {
        if self.away_since.is_some() || self.paused_until.is_some() {
            return Vec::new();
        }
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
        if self.away_since.is_some() {
            return "Paused while you're away".into();
        }
        if let Some(until) = self.paused_until {
            return format!("Paused until {}", self.describe_until(until, now));
        }
        let enabled = || self.entries.iter().filter(|e| e.reminder.enabled);
        match self.next_up() {
            Some((e, due)) => format!(
                "Next: {} {} {}",
                e.reminder.emoji,
                e.reminder.title,
                format_remaining(due.saturating_sub(now), self.minute_ms)
            ),
            None if enabled().any(|e| e.showing) => "Reminder on screen".into(),
            None if enabled().next().is_some() => "Outside active hours".into(),
            None => "All reminders are off".into(),
        }
    }
}

impl Scheduler {
    /// Tray menu rows: (id, "💧 Drink water", time until it next fires).
    /// No time while paused or away, or for reminders that are off.
    pub fn menu_rows(&self, now: u64) -> Vec<MenuRow> {
        let quiet = self.away_since.is_some() || self.paused_until.is_some();
        self.entries
            .iter()
            .map(|e| {
                let r = &e.reminder;
                let time_left = if !r.enabled || quiet {
                    None
                } else if e.showing {
                    Some("now".to_string())
                } else {
                    self.projected_due(e)
                        .map(|due| short_remaining(due.saturating_sub(now), self.minute_ms))
                };
                MenuRow {
                    id: r.id.clone(),
                    name: format!("{} {}", r.emoji, r.title),
                    time_left,
                }
            })
            .collect()
    }

    /// When a reminder will actually pop up next: its next due time, rolled
    /// forward by its interval past any stretch outside its active hours
    /// (mirroring `tick`). None if that doesn't happen within a week.
    fn projected_due(&self, entry: &Entry) -> Option<u64> {
        const WEEK_MINUTES: u64 = 7 * 24 * 60;
        let step = entry.reminder.interval_minutes.max(1) * self.minute_ms;
        let limit = entry.next_due + WEEK_MINUTES * self.minute_ms;
        let mut due = entry.next_due;
        while due <= limit {
            if entry.reminder.is_active_at((self.local)(due)) {
                return Some(due);
            }
            due += step;
        }
        None
    }

    fn next_up(&self) -> Option<(&Entry, u64)> {
        self.entries
            .iter()
            .filter(|e| e.reminder.enabled && !e.showing)
            .filter_map(|e| self.projected_due(e).map(|due| (e, due)))
            .min_by_key(|(_, due)| *due)
    }

    /// "14:30", or "tomorrow" for a pause that ends at midnight.
    fn describe_until(&self, until: u64, now: u64) -> String {
        let (end, today) = ((self.local)(until), (self.local)(now));
        if end.minute == 0 && end.weekday != today.weekday {
            "tomorrow".into()
        } else {
            format!("{:02}:{:02}", end.minute / 60, end.minute % 60)
        }
    }
}

fn format_remaining(ms: u64, minute_ms: u64) -> String {
    match short_remaining(ms, minute_ms).as_str() {
        "now" => "now".into(),
        short => format!("in {short}"),
    }
}

/// "now", "7m", "1h", "1h 5m" (minutes rounded up).
fn short_remaining(ms: u64, minute_ms: u64) -> String {
    let minutes = ms.div_ceil(minute_ms);
    match (minutes / 60, minutes % 60) {
        (0, 0) => "now".into(),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
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

pub fn init(app: &AppHandle, reminders: Vec<Reminder>, paused_until: Option<u64>) {
    let minute_ms = if fast_mode() { 1_000 } else { 60_000 };
    let mut scheduler = Scheduler::new(reminders, minute_ms, now_ms());
    if let Some(until) = paused_until {
        scheduler.pause(until);
    }
    app.manage::<SchedulerState>(Mutex::new(scheduler));
}

/// Epoch ms of the next local midnight.
pub fn next_midnight_ms() -> u64 {
    let tomorrow = Local::now().date_naive().succ_opt().unwrap_or_default();
    tomorrow
        .and_hms_opt(0, 0, 0)
        .and_then(|t| Local.from_local_datetime(&t).earliest())
        .map_or_else(|| now_ms() + 86_400_000, |t| t.timestamp_millis() as u64)
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

/// A gap this long between ticks means the Mac was asleep.
const SLEEP_GAP_MS: u64 = 2 * 60_000;

fn run_tick(app: &AppHandle) {
    static LAST_TICK: AtomicU64 = AtomicU64::new(0);
    let now = now_ms();
    let last = LAST_TICK.swap(now, Ordering::Relaxed);
    let idle_limit_secs = crate::settings::general(app).idle_pause_minutes * 60;
    let away = crate::idle::presence().is_away(idle_limit_secs);

    let (due, went_away, pause_ended) = {
        let state = app.state::<SchedulerState>();
        let mut scheduler = state.lock().unwrap();
        // Sleep counts as time away, with the same rules as walking off.
        // (Already away: coming back will cover the sleep too.)
        let slept = last > 0 && now.saturating_sub(last) > SLEEP_GAP_MS;
        if slept && idle_limit_secs > 0 && !scheduler.is_away() {
            scheduler.return_from_break(last, now);
        }
        let went_away = scheduler.set_away(away, now) && away;
        let pause_ended = scheduler.expire_pause(now);
        (scheduler.tick(now), went_away, pause_ended)
    };

    if went_away {
        popup::dismiss_all(app);
    }
    if pause_ended {
        crate::settings::pause_changed(app);
    }
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
    let (status, rows) = {
        let state = app.state::<SchedulerState>();
        let scheduler = state.lock().unwrap();
        let now = now_ms();
        (scheduler.status_text(now), scheduler.menu_rows(now))
    };
    crate::tray::set_status(app, &status, &rows);
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
        assert_eq!(s.status_text(90 * MIN), "Next: 💧 Drink water in 6h 30m");
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
    fn pause_blocks_then_resume_restarts_intervals() {
        let mut s = one(10);
        s.pause(30 * MIN);
        assert!(s.tick(10 * MIN).is_empty());
        assert!(s.tick(20 * MIN).is_empty());
        assert!(!s.expire_pause(29 * MIN));
        assert!(s.expire_pause(30 * MIN));
        assert!(s.paused_until().is_none());
        assert!(s.tick(39 * MIN).is_empty(), "fresh interval after pause");
        assert_eq!(s.tick(40 * MIN).len(), 1);
    }

    #[test]
    fn manual_resume_restarts_and_clears_showing() {
        let mut s = one(10);
        s.tick(10 * MIN); // showing
        s.pause(100 * MIN);
        s.resume(15 * MIN);
        assert_eq!(s.tick(25 * MIN).len(), 1);
    }

    fn next_due(s: &Scheduler, id: &str) -> u64 {
        s.entries
            .iter()
            .find(|e| e.reminder.id == id)
            .unwrap()
            .next_due
    }

    #[test]
    fn away_blocks_popups() {
        let mut s = one(10);
        assert!(s.set_away(true, 5 * MIN));
        assert!(!s.set_away(true, 6 * MIN), "no change");
        assert!(s.tick(10 * MIN).is_empty());
        assert_eq!(s.status_text(10 * MIN), "Paused while you're away");
    }

    #[test]
    fn return_restarts_eyes_and_stand_but_resumes_water() {
        let mut s = scheduler(presets()); // water 45, eyes 20, stand 60
        s.set_away(true, 10 * MIN);
        s.set_away(false, 40 * MIN); // away 30 min
                                     // Water had 35 min left when you left, so it still has 35.
        assert_eq!(next_due(&s, "water"), 75 * MIN);
        // Eyes and stand start fresh from your return.
        assert_eq!(next_due(&s, "eyes"), 60 * MIN);
        assert_eq!(next_due(&s, "stand"), 100 * MIN);
    }

    #[test]
    fn popup_closed_while_away_returns_when_resumed() {
        let mut water = presets().remove(0);
        water.interval_minutes = 10;
        let mut s = scheduler(vec![water]);
        assert_eq!(s.tick(10 * MIN).len(), 1); // on screen
        s.set_away(true, 11 * MIN); // closed because you left
        s.set_away(false, 30 * MIN);
        // It was due when you left, so it's due again on return.
        assert_eq!(s.tick(30 * MIN).len(), 1);
    }

    #[test]
    fn sleep_gap_uses_break_rules() {
        let mut s = scheduler(presets());
        s.return_from_break(5 * MIN, 125 * MIN); // lid closed for 2 hours
        assert_eq!(next_due(&s, "water"), 165 * MIN);
        assert_eq!(next_due(&s, "eyes"), 145 * MIN);
    }

    #[test]
    fn pause_status_text() {
        let mut s = one(10);
        s.pause((14 * 60 + 30) * MIN);
        assert_eq!(s.status_text(13 * 60 * MIN), "Paused until 14:30");
        s.pause(24 * 60 * MIN); // Tuesday 00:00
        assert_eq!(s.status_text(20 * 60 * MIN), "Paused until tomorrow");
    }

    fn times(s: &Scheduler, now: u64) -> Vec<Option<String>> {
        s.menu_rows(now).into_iter().map(|r| r.time_left).collect()
    }

    #[test]
    fn menu_rows_show_time_left_per_reminder() {
        let mut s = scheduler(presets());
        let rows = s.menu_rows(8 * MIN);
        assert_eq!(rows[0].name, "💧 Drink water");
        assert_eq!(
            times(&s, 8 * MIN),
            [Some("37m".into()), Some("12m".into()), Some("52m".into())]
        );
        s.tick(20 * MIN);
        assert_eq!(times(&s, 20 * MIN)[1].as_deref(), Some("now"));
        s.pause(100 * MIN);
        assert_eq!(times(&s, 21 * MIN), [None, None, None]);
    }

    #[test]
    fn menu_rows_project_past_inactive_hours() {
        let mut off = presets().remove(0);
        off.enabled = false;
        let mut eyes = presets().remove(1); // 08:00–20:00
        eyes.interval_minutes = 60; // first due Monday 01:00, outside hours
        let mut s = Scheduler::new(vec![off, eyes], MIN, 0);
        s.local = test_local;
        // Rolls hourly to 08:00: 8h from midnight.
        assert_eq!(times(&s, 0), [None, Some("8h".into())]);
        assert_eq!(s.status_text(0), "Next: 👀 Rest your eyes in 8h");
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
