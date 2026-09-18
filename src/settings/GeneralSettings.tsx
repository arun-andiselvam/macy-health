import { useId } from "react";
import type { General } from "../types";

const AWAY_CHOICES = [
  { value: 0, label: "Never" },
  { value: 2, label: "After 2 minutes" },
  { value: 5, label: "After 5 minutes" },
  { value: 10, label: "After 10 minutes" },
  { value: 15, label: "After 15 minutes" },
];

const POPUP_CHOICES = [
  { value: 15, label: "15 seconds" },
  { value: 30, label: "30 seconds" },
  { value: 60, label: "1 minute" },
  { value: 120, label: "2 minutes" },
];

type Props = {
  general: General;
  onChange: (next: Pick<General, "launchAtLogin" | "idlePauseMinutes" | "popupSeconds">) => void;
  onPause: (minutes: number | null) => void;
  onResume: () => void;
};

export function describePause(until: number) {
  const end = new Date(until);
  const now = new Date();
  const isMidnight = end.getHours() === 0 && end.getMinutes() === 0 && end.getDate() !== now.getDate();
  if (isMidnight) return "tomorrow";
  return end.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}

export function GeneralSettings({ general, onChange, onPause, onResume }: Props) {
  const id = useId();
  const { launchAtLogin, idlePauseMinutes, popupSeconds, pausedUntil } = general;
  const base = { launchAtLogin, idlePauseMinutes, popupSeconds };

  return (
    <ul className="group general">
      <li className="setting">
        <div className="setting-text">
          <span className="setting-title" id={`${id}-pause`}>
            Pause all reminders
          </span>
          <span className="setting-help">
            {pausedUntil ? `Paused until ${describePause(pausedUntil)}.` : "Take a break from every reminder at once."}
          </span>
        </div>
        <div className="setting-control" role="group" aria-labelledby={`${id}-pause`}>
          {pausedUntil ? (
            <button type="button" className="button primary" onClick={onResume}>
              Resume now
            </button>
          ) : (
            <>
              <button type="button" className="button" onClick={() => onPause(30)}>
                30 min
              </button>
              <button type="button" className="button" onClick={() => onPause(60)}>
                1 hour
              </button>
              <button type="button" className="button" onClick={() => onPause(null)}>
                Until tomorrow
              </button>
            </>
          )}
        </div>
      </li>

      <li className="setting">
        <div className="setting-text">
          <label className="setting-title" htmlFor={`${id}-away`}>
            Pause while I'm away
          </label>
          <span className="setting-help">
            Stops reminders when you haven't touched the keyboard or mouse, and starts them fresh when you're back.
          </span>
        </div>
        <div className="setting-control">
          <select
            id={`${id}-away`}
            className="input"
            value={idlePauseMinutes}
            onChange={(e) => onChange({ ...base, idlePauseMinutes: Number(e.target.value) })}
          >
            {AWAY_CHOICES.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </div>
      </li>

      <li className="setting">
        <div className="setting-text">
          <label className="setting-title" htmlFor={`${id}-popup`}>
            Keep popups on screen for
          </label>
          <span className="setting-help">After this, a popup closes itself and the reminder counts as missed.</span>
        </div>
        <div className="setting-control">
          <select
            id={`${id}-popup`}
            className="input"
            value={popupSeconds}
            onChange={(e) => onChange({ ...base, popupSeconds: Number(e.target.value) })}
          >
            {POPUP_CHOICES.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </div>
      </li>

      <li className="setting">
        <div className="setting-text">
          <span className="setting-title" id={`${id}-login`}>
            Open at login
          </span>
          <span className="setting-help">Start Macy Health in the menu bar when you log in.</span>
        </div>
        <div className="setting-control">
          <button
            type="button"
            role="switch"
            className="switch"
            aria-checked={launchAtLogin}
            aria-labelledby={`${id}-login`}
            onClick={() => onChange({ ...base, launchAtLogin: !launchAtLogin })}
          />
        </div>
      </li>
    </ul>
  );
}
