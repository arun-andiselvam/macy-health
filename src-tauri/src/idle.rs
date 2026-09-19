//! Is the user at the computer? Combines input idle time with, on macOS,
//! screen lock, display sleep and "keep the display on" requests from apps
//! (video players and calls), so watching a video doesn't count as away.

/// Seconds since the last keyboard or mouse input, system-wide.
pub fn idle_seconds() -> u64 {
    user_idle::UserIdle::get_time().map_or(0, |t| t.as_seconds())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Presence {
    pub idle_seconds: u64,
    pub screen_locked: bool,
    pub display_asleep: bool,
    /// An app asked macOS to keep the display on (video, call, presentation).
    pub display_kept_on: bool,
}

impl Presence {
    /// Away when the screen is locked or asleep, or after `idle_limit_secs`
    /// without input unless something is keeping the display on.
    /// `idle_limit_secs == 0` turns away detection off entirely.
    pub fn is_away(&self, idle_limit_secs: u64) -> bool {
        if idle_limit_secs == 0 {
            return false;
        }
        self.screen_locked
            || self.display_asleep
            || (self.idle_seconds >= idle_limit_secs && !self.display_kept_on)
    }
}

// On macOS every field is set; other platforms rely on the defaults.
#[allow(clippy::needless_update)]
pub fn presence() -> Presence {
    Presence {
        idle_seconds: idle_seconds(),
        #[cfg(target_os = "macos")]
        screen_locked: mac::screen_locked(),
        #[cfg(target_os = "macos")]
        display_asleep: mac::display_asleep(),
        #[cfg(target_os = "macos")]
        display_kept_on: mac::display_kept_on(),
        ..Presence::default()
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ptr;

    use objc2::{rc::Retained, runtime::AnyObject};
    use objc2_foundation::{NSDictionary, NSNumber, NSString};

    type Dict = NSDictionary<NSString, AnyObject>;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGSessionCopyCurrentDictionary() -> *mut Dict;
        fn CGMainDisplayID() -> u32;
        fn CGDisplayIsAsleep(display: u32) -> u32;
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMCopyAssertionsStatus(status: *mut *mut Dict) -> i32;
    }

    /// Takes ownership of a +1 CFDictionary (toll-free bridged to NSDictionary).
    fn owned(dict: *mut Dict) -> Option<Retained<Dict>> {
        unsafe { Retained::from_raw(dict) }
    }

    fn number(dict: &Dict, key: &str) -> Option<isize> {
        let value = dict.objectForKey(&NSString::from_str(key))?;
        Some(value.downcast::<NSNumber>().ok()?.integerValue())
    }

    pub fn screen_locked() -> bool {
        owned(unsafe { CGSessionCopyCurrentDictionary() })
            .and_then(|d| number(&d, "CGSSessionScreenIsLocked"))
            .is_some_and(|v| v != 0)
    }

    pub fn display_asleep() -> bool {
        unsafe { CGDisplayIsAsleep(CGMainDisplayID()) != 0 }
    }

    pub fn display_kept_on() -> bool {
        let mut status = ptr::null_mut();
        if unsafe { IOPMCopyAssertionsStatus(&mut status) } != 0 {
            return false;
        }
        owned(status)
            .and_then(|d| number(&d, "PreventUserIdleDisplaySleep"))
            .is_some_and(|v| v > 0)
    }
}

#[cfg(test)]
mod presence_tests {
    use super::Presence;

    const FIVE_MIN: u64 = 300;

    #[test]
    fn locked_or_asleep_is_away_at_once() {
        let locked = Presence {
            screen_locked: true,
            ..Default::default()
        };
        let asleep = Presence {
            display_asleep: true,
            ..Default::default()
        };
        assert!(locked.is_away(FIVE_MIN));
        assert!(asleep.is_away(FIVE_MIN));
    }

    #[test]
    fn idle_is_away_unless_display_kept_on() {
        let idle = Presence {
            idle_seconds: 301,
            ..Default::default()
        };
        assert!(idle.is_away(FIVE_MIN));
        let watching = Presence {
            display_kept_on: true,
            ..idle
        };
        assert!(!watching.is_away(FIVE_MIN));
        let active = Presence {
            idle_seconds: 10,
            ..Default::default()
        };
        assert!(!active.is_away(FIVE_MIN));
    }

    #[test]
    fn never_setting_disables_everything() {
        let locked = Presence {
            screen_locked: true,
            idle_seconds: 9999,
            ..Default::default()
        };
        assert!(!locked.is_away(0));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    /// Manual check of the live signals: `cargo test -- --ignored --nocapture presence`.
    #[test]
    #[ignore]
    fn presence_live() {
        println!("{:?}", super::presence());
    }

    /// Manual check against IOKit's own counter: `cargo test -- --ignored idle`.
    #[test]
    #[ignore]
    fn matches_ioreg() {
        let out = std::process::Command::new("ioreg")
            .args(["-c", "IOHIDSystem"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let ns: u64 = text
            .lines()
            .find_map(|l| l.split("\"HIDIdleTime\" = ").nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap();
        let expected = ns / 1_000_000_000;
        let actual = super::idle_seconds();
        println!("ioreg: {expected}s, idle_seconds: {actual}s");
        assert!(actual.abs_diff(expected) <= 2);
    }
}
