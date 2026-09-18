# Macy Health

A menu-bar app that reminds you to drink water, rest your eyes and stand up, on a schedule you choose. Built with [Tauri](https://tauri.app), Rust and React.

- Lives only in the menu bar (system tray on Windows and Linux); there's no Dock icon.
- Reminders pop up in the top-right corner with a short animation, and never steal focus from what you're doing.
- Each reminder has its own interval, active hours and days. You can add your own.
- Pause for a while, or automatically while you're away from the keyboard.
- Updates itself from GitHub Releases.

## Download

Get the latest installer from [Releases](https://github.com/arun-andiselvam/macy-health/releases/latest):

| System | File |
|---|---|
| macOS 11+ (Apple Silicon and Intel) | `Macy-Health_<version>_universal.dmg` |
| Windows 10+ | `Macy.Health_<version>_x64-setup.exe` |
| Linux | `.AppImage` or `.deb` |

The macOS build is signed and notarized. The Windows build isn't code-signed yet, so SmartScreen shows a warning on first run (More info → Run anyway).

## Development

Requirements: Node 22+, pnpm 10, and Rust (stable).

```sh
pnpm install
pnpm tauri dev                # run the app
MACY_FAST=1 pnpm tauri dev    # minutes become seconds, to test schedules quickly
cd src-tauri && cargo test    # scheduler, validation and settings tests
```

Settings are saved to `settings.json` in the app config folder (`~/Library/Application Support/com.arun.macyhealth/` on macOS).

### Layout

| Path | What's there |
|---|---|
| `src-tauri/src/scheduler.rs` | When reminders fire: intervals, active hours, snooze, pause, away, sleep/wake |
| `src-tauri/src/popup.rs` | The popup window: position, queue, show and hide |
| `src-tauri/src/settings.rs` | Saving settings, and the commands the Settings window calls |
| `src-tauri/src/tray.rs` | Menu-bar menu |
| `src-tauri/src/updater.rs` | Update checks and installs |
| `src/popup/` | Popup UI and the animated illustrations |
| `src/settings/`, `src/App.tsx` | Settings window |
| `scripts/` | Icon and sound generators, macOS release script |

## Releasing

1. Set the new version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`, then commit.
2. Tag and push it. GitHub Actions builds the Windows and Linux installers into a draft release:
   ```sh
   git tag v0.2.0 && git push origin v0.2.0
   ```
3. On the Mac with the signing setup, build, notarize and publish:
   ```sh
   pnpm publish:mac
   ```
   This uploads the macOS files, adds them to `latest.json` for the updater, and publishes the release.

One-time setup for the release Mac: the HITASOFT Developer ID certificate in the login keychain, the `hitasoft-notary` notarytool profile, the updater key at `~/.tauri/macy-health.key` (password in the keychain under `macy-health-updater-key`), and `gh auth login`. The repository needs the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets for the CI builds.

`pnpm release:mac` does everything except upload, if you only want a local notarized build.
