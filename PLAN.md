# Macy Health — Plan

Tray-only reminder app (water, eye rest, stand up, custom). Tauri v2, macOS first, then Windows/Linux.

- App name: Macy Health
- Bundle ID: `com.arun.macyhealth`
- Frontend: React + TypeScript + Vite
- Reminder style: custom animated popup, top-right corner

## Architecture

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri v2 | Cross-platform from one codebase |
| Tray-only | `ActivationPolicy::Accessory` (macOS), `skip_taskbar` (Windows) | No Dock/taskbar icon |
| Scheduler | Rust (tokio) | Webview may be throttled when hidden |
| Timer logic | Persist `nextDueAt`, tick every 15s | Survives sleep/wake without drift |
| Reminder UI | Custom popup window (see below) | Animation + action buttons |
| Storage | `tauri-plugin-store` (JSON) | Simple, cross-platform |
| Autostart | `tauri-plugin-autostart` | Launch at login |
| Single instance | `tauri-plugin-single-instance` | No double popups |
| Idle | `user-idle` crate | Pause while away, reset on return |
| Animations | `lottie-react` + bundled Lottie JSON | Rich, small, offline |

## Windows (Tauri)

| Label | Purpose | Config |
|---|---|---|
| `settings` | Manage reminders & global settings | Normal window, hidden until opened from tray |
| `popup` | Reminder popup | Transparent, no decorations, always-on-top, skip taskbar, non-focusable, all workspaces, ~360×140, created hidden at startup |

## Popup behaviour

- Position: top-right of the work area (below menu bar) on the monitor containing the cursor; 16px margin.
- Flow: Rust scheduler fires → positions + shows `popup` → emits `reminder:show` with the reminder payload → React renders it.
- Content: Lottie animation per reminder type + title + message.
  - 💧 Water: glass filling
  - 👀 Eye rest: blinking eye + 20s countdown ring (20-20-20 rule)
  - 🧍 Stand up: figure stretching
  - Custom: chosen emoji with bounce animation
- Actions: **Done**, **Snooze 5m**, **Skip** → `invoke("reminder_action", { id, action })`.
- Auto-dismiss after 30s (configurable) → logged as missed, next cycle scheduled.
- Enter/exit animation: slide in from right + fade.
- Multiple due at once: queue, show one at a time.
- Must not steal focus from the active app.

## Reminder model

```ts
Reminder {
  id: string
  kind: "water" | "eyes" | "stand" | "custom"
  title: string
  message: string
  emoji: string
  intervalMinutes: number
  enabled: boolean
  activeHours: { start: "09:00", end: "18:00" }
  days: number[]          // 1..7
  sound: boolean
  nextDueAt?: string      // runtime, persisted
}

GlobalSettings {
  launchAtLogin: boolean
  idlePauseMinutes: number
  popupDurationSeconds: number
  pausedUntil?: string
}
```

Presets: 💧 Drink water (45m), 👀 Eye rest (20m), 🧍 Stand up (60m).

## Tray menu

- Next: 💧 Water in 12m (macOS: also as menu bar title)
- Pause 30m / 1h / until tomorrow · Resume
- Per-reminder toggles
- Test popup (dev only)
- Settings…
- Quit

## Phases

| # | Scope | Done when |
|---|---|---|
| 0 | Install Rust + Xcode CLT, scaffold `pnpm create tauri-app` (React-TS) | Empty app runs |
| 1 | Tray icon, hide Dock, tray menu, Quit | Lives only in menu bar |
| 2 | Popup window: transparent, top-right, no focus steal, slide animation, Done/Snooze/Skip; "Test popup" from tray | Popup looks right on 1 and 2 monitors |
| 3 | Rust scheduler, presets hard-coded, fires popup | Fires on time incl. after sleep/wake |
| 4 | Lottie animations per kind, eye-rest countdown, sound | Each preset has its animation |
| 5 | Settings window: CRUD, interval, active hours, days, persistence | Survives restart |
| 6 | Pause/snooze, idle detection, launch at login, single instance | Daily-drivable |
| 7 | Sign + notarize `.dmg`, auto-updater | Shareable build |
| 8 | Windows + Linux via GitHub Actions matrix | `.msi` / `.AppImage` |
| Later | Full-screen break overlay, stats/streaks, popup position setting, skip in DND/fullscreen | — |

## Risks

- **macOS over full-screen apps:** a plain Tauri window won't show above full-screen Spaces. Needs `tauri-nspanel` (NSPanel, non-activating + full-screen-auxiliary). Test in Phase 2.
- **Transparency on macOS:** requires `macOSPrivateApi: true` in `tauri.conf.json` (blocks Mac App Store; fine for direct distribution).
- **Linux:** tray support varies by desktop (GNOME needs AppIndicator extension); transparent windows depend on compositor.
- **Windows:** top-right is unusual there (notifications appear bottom-right); make position configurable later.
