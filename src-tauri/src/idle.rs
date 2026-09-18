//! Seconds since the last keyboard or mouse input, system-wide.

pub fn idle_seconds() -> u64 {
    user_idle::UserIdle::get_time().map_or(0, |t| t.as_seconds())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
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
