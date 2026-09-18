//! Reminder model, presets and validation.

use serde::{Deserialize, Serialize};

use crate::popup::PopupReminder;

pub const MIN_INTERVAL_MINUTES: u64 = 1;
pub const MAX_INTERVAL_MINUTES: u64 = 8 * 60;
const MAX_TITLE_CHARS: usize = 60;
const MAX_MESSAGE_CHARS: usize = 160;
const MAX_EMOJI_CHARS: usize = 16;

pub const CUSTOM_KIND: &str = "custom";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub kind: String,
    pub emoji: String,
    pub title: String,
    pub message: String,
    pub interval_minutes: u64,
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub sound: bool,
    /// "HH:MM", local time. Equal start and end means all day.
    #[serde(default = "default_start")]
    pub active_start: String,
    #[serde(default = "default_end")]
    pub active_end: String,
    /// ISO weekdays, 1 = Monday … 7 = Sunday.
    #[serde(default = "every_day")]
    pub days: Vec<u8>,
}

fn default_true() -> bool {
    true
}
fn default_start() -> String {
    "08:00".into()
}
fn default_end() -> String {
    "20:00".into()
}
fn every_day() -> Vec<u8> {
    (1..=7).collect()
}

impl Reminder {
    pub fn to_popup(&self) -> PopupReminder {
        PopupReminder {
            id: self.id.clone(),
            kind: self.kind.clone(),
            emoji: self.emoji.clone(),
            title: self.title.clone(),
            message: self.message.clone(),
            sound: self.sound,
        }
    }

    pub fn is_custom(&self) -> bool {
        self.kind == CUSTOM_KIND
    }

    /// Whether this reminder may fire at the given local time.
    pub fn is_active_at(&self, local: LocalTime) -> bool {
        if !self.days.contains(&local.weekday) {
            return false;
        }
        let (Some(start), Some(end)) =
            (parse_hhmm(&self.active_start), parse_hhmm(&self.active_end))
        else {
            return true;
        };
        match start.cmp(&end) {
            std::cmp::Ordering::Equal => true,
            std::cmp::Ordering::Less => (start..end).contains(&local.minute),
            // Window crosses midnight, e.g. 22:00–02:00.
            std::cmp::Ordering::Greater => local.minute >= start || local.minute < end,
        }
    }

    /// Trims and checks user-editable fields.
    pub fn validate(&mut self) -> Result<(), String> {
        self.title = self.title.trim().to_string();
        self.message = self.message.trim().to_string();
        self.emoji = self.emoji.trim().to_string();

        if self.title.is_empty() {
            return Err("Give the reminder a title.".into());
        }
        if self.title.chars().count() > MAX_TITLE_CHARS {
            return Err(format!(
                "Keep the title under {MAX_TITLE_CHARS} characters."
            ));
        }
        if self.message.chars().count() > MAX_MESSAGE_CHARS {
            return Err(format!(
                "Keep the message under {MAX_MESSAGE_CHARS} characters."
            ));
        }
        if self.emoji.is_empty() || self.emoji.chars().count() > MAX_EMOJI_CHARS {
            return Err("Pick an emoji for the reminder.".into());
        }
        if !(MIN_INTERVAL_MINUTES..=MAX_INTERVAL_MINUTES).contains(&self.interval_minutes) {
            return Err(format!(
                "Choose an interval between {MIN_INTERVAL_MINUTES} minute and {} hours.",
                MAX_INTERVAL_MINUTES / 60
            ));
        }
        if parse_hhmm(&self.active_start).is_none() || parse_hhmm(&self.active_end).is_none() {
            return Err("Active hours must be times like 09:00.".into());
        }
        self.days.retain(|d| (1..=7).contains(d));
        self.days.sort_unstable();
        self.days.dedup();
        if self.days.is_empty() {
            return Err("Pick at least one day.".into());
        }
        Ok(())
    }
}

/// Local wall-clock position used for active-hours checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    /// 1 = Monday … 7 = Sunday.
    pub weekday: u8,
    /// Minutes since local midnight.
    pub minute: u16,
}

pub fn parse_hhmm(s: &str) -> Option<u16> {
    let (h, m) = s.split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub fn presets() -> Vec<Reminder> {
    let preset = |id: &str, emoji: &str, title: &str, message: &str, interval_minutes| Reminder {
        id: id.into(),
        kind: id.into(),
        emoji: emoji.into(),
        title: title.into(),
        message: message.into(),
        interval_minutes,
        enabled: true,
        sound: true,
        active_start: default_start(),
        active_end: default_end(),
        days: every_day(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(weekday: u8, hhmm: &str) -> LocalTime {
        LocalTime {
            weekday,
            minute: parse_hhmm(hhmm).unwrap(),
        }
    }

    fn water() -> Reminder {
        presets().remove(0)
    }

    #[test]
    fn parses_times() {
        assert_eq!(parse_hhmm("00:00"), Some(0));
        assert_eq!(parse_hhmm("09:30"), Some(570));
        assert_eq!(parse_hhmm("23:59"), Some(1439));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("9"), None);
        assert_eq!(parse_hhmm("ab:cd"), None);
    }

    #[test]
    fn daytime_window() {
        let r = water(); // 08:00–20:00
        assert!(!r.is_active_at(at(1, "07:59")));
        assert!(r.is_active_at(at(1, "08:00")));
        assert!(r.is_active_at(at(1, "19:59")));
        assert!(!r.is_active_at(at(1, "20:00")));
    }

    #[test]
    fn overnight_window() {
        let mut r = water();
        r.active_start = "22:00".into();
        r.active_end = "02:00".into();
        assert!(r.is_active_at(at(1, "23:00")));
        assert!(r.is_active_at(at(1, "01:30")));
        assert!(!r.is_active_at(at(1, "12:00")));
    }

    #[test]
    fn equal_start_end_is_all_day() {
        let mut r = water();
        r.active_start = "00:00".into();
        r.active_end = "00:00".into();
        assert!(r.is_active_at(at(1, "03:00")));
    }

    #[test]
    fn respects_days() {
        let mut r = water();
        r.days = vec![1, 2, 3, 4, 5];
        assert!(r.is_active_at(at(5, "10:00")));
        assert!(!r.is_active_at(at(6, "10:00")));
    }

    #[test]
    fn validation() {
        let mut r = water();
        r.title = "  Hydrate  ".into();
        r.days = vec![3, 1, 1, 9];
        assert!(r.validate().is_ok());
        assert_eq!(r.title, "Hydrate");
        assert_eq!(r.days, vec![1, 3]);

        let mut bad = water();
        bad.title = "  ".into();
        assert!(bad.validate().is_err());

        let mut bad = water();
        bad.interval_minutes = 0;
        assert!(bad.validate().is_err());

        let mut bad = water();
        bad.days = vec![];
        assert!(bad.validate().is_err());

        let mut bad = water();
        bad.active_start = "25:00".into();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn missing_fields_get_defaults() {
        let json = r#"{"id":"x","kind":"custom","emoji":"🧘","title":"Breathe",
            "message":"","intervalMinutes":30,"enabled":true}"#;
        let r: Reminder = serde_json::from_str(json).unwrap();
        assert!(r.sound);
        assert_eq!(r.active_start, "08:00");
        assert_eq!(r.days.len(), 7);
    }
}
