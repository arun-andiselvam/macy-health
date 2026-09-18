import { useId, useState } from "react";
import type { Reminder } from "../types";
import { DAY_INITIALS, DAY_NAMES, isAllDay } from "./format";

const EMOJI_SUGGESTIONS = ["🧘", "🚶", "🫁", "🍎", "💊", "☀️", "📵", "🙆"];

type Props = {
  draft: Reminder;
  onChange: (draft: Reminder) => void;
  onSave: () => void;
  onCancel: () => void;
  onDelete?: () => void;
  onPreview: () => void;
  saving: boolean;
  error: string | null;
};

export function ReminderEditor({ draft, onChange, onSave, onCancel, onDelete, onPreview, saving, error }: Props) {
  const id = useId();
  const isNew = draft.id === "";
  const isCustom = draft.kind === "custom";
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [unit, setUnit] = useState<"min" | "hours">(
    draft.intervalMinutes >= 60 && draft.intervalMinutes % 60 === 0 ? "hours" : "min",
  );

  const set = <K extends keyof Reminder>(key: K, value: Reminder[K]) => onChange({ ...draft, [key]: value });

  const intervalValue = unit === "hours" ? draft.intervalMinutes / 60 : draft.intervalMinutes;
  const setInterval = (value: number, nextUnit = unit) => {
    if (!Number.isFinite(value)) return;
    set("intervalMinutes", Math.round(nextUnit === "hours" ? value * 60 : value));
  };

  const allDay = isAllDay(draft);
  const toggleDay = (day: number) =>
    set("days", draft.days.includes(day) ? draft.days.filter((d) => d !== day) : [...draft.days, day].sort());

  return (
    <form
      className="editor"
      onSubmit={(e) => {
        e.preventDefault();
        onSave();
      }}
    >
      {isCustom && (
        <div className="field">
          <label htmlFor={`${id}-emoji`}>Emoji</label>
          <div className="emoji-field">
            <input
              id={`${id}-emoji`}
              className="input emoji-input"
              value={draft.emoji}
              maxLength={8}
              onChange={(e) => set("emoji", e.target.value)}
            />
            <div className="emoji-suggestions" role="group" aria-label="Suggested emoji">
              {EMOJI_SUGGESTIONS.map((emoji) => (
                <button
                  key={emoji}
                  type="button"
                  className={`emoji-choice${draft.emoji === emoji ? " is-selected" : ""}`}
                  aria-pressed={draft.emoji === emoji}
                  onClick={() => set("emoji", emoji)}
                >
                  {emoji}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      <div className="field">
        <label htmlFor={`${id}-title`}>Title</label>
        <input
          id={`${id}-title`}
          className="input"
          value={draft.title}
          maxLength={60}
          placeholder="Breathe"
          autoFocus={isNew}
          onChange={(e) => set("title", e.target.value)}
        />
      </div>

      <div className="field">
        <label htmlFor={`${id}-message`}>Message</label>
        <textarea
          id={`${id}-message`}
          className="input"
          rows={2}
          maxLength={160}
          value={draft.message}
          placeholder="Take three slow, deep breaths."
          onChange={(e) => set("message", e.target.value)}
        />
      </div>

      <div className="field">
        <label htmlFor={`${id}-interval`}>Repeat every</label>
        <div className="inline">
          <input
            id={`${id}-interval`}
            className="input number"
            type="number"
            min={1}
            max={unit === "hours" ? 8 : 480}
            step={unit === "hours" ? 0.5 : 1}
            value={intervalValue}
            onChange={(e) => setInterval(e.target.valueAsNumber)}
          />
          <select
            className="input"
            aria-label="Interval unit"
            value={unit}
            onChange={(e) => {
              const next = e.target.value as "min" | "hours";
              setUnit(next);
              setInterval(intervalValue, next);
            }}
          >
            <option value="min">minutes</option>
            <option value="hours">hours</option>
          </select>
        </div>
      </div>

      <div className="field">
        <span className="label" id={`${id}-hours`}>
          Active hours
        </span>
        <div className="inline" role="group" aria-labelledby={`${id}-hours`}>
          <input
            className="input"
            type="time"
            aria-label="From"
            value={allDay ? "00:00" : draft.activeStart}
            disabled={allDay}
            onChange={(e) => e.target.value && set("activeStart", e.target.value)}
          />
          <span className="muted">to</span>
          <input
            className="input"
            type="time"
            aria-label="Until"
            value={allDay ? "00:00" : draft.activeEnd}
            disabled={allDay}
            onChange={(e) => e.target.value && set("activeEnd", e.target.value)}
          />
          <label className="check">
            <input
              type="checkbox"
              checked={allDay}
              onChange={(e) =>
                onChange(
                  e.target.checked
                    ? { ...draft, activeStart: "00:00", activeEnd: "00:00" }
                    : { ...draft, activeStart: "08:00", activeEnd: "20:00" },
                )
              }
            />
            All day
          </label>
        </div>
      </div>

      <div className="field">
        <span className="label" id={`${id}-days`}>
          Days
        </span>
        <div className="days" role="group" aria-labelledby={`${id}-days`}>
          {DAY_INITIALS.map((initial, i) => {
            const day = i + 1;
            const on = draft.days.includes(day);
            return (
              <button
                key={day}
                type="button"
                className={`day${on ? " is-on" : ""}`}
                aria-pressed={on}
                aria-label={DAY_NAMES[i]}
                title={DAY_NAMES[i]}
                onClick={() => toggleDay(day)}
              >
                {initial}
              </button>
            );
          })}
        </div>
      </div>

      <div className="field">
        <span className="label" aria-hidden="true" />
        <label className="check">
          <input type="checkbox" checked={draft.sound} onChange={(e) => set("sound", e.target.checked)} />
          Play a sound
        </label>
      </div>

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      <div className="editor-actions">
        <button type="button" className="button" onClick={onPreview}>
          Preview
        </button>
        {onDelete && (
          <button
            type="button"
            className={`button danger${confirmingDelete ? " is-confirming" : ""}`}
            onClick={() => (confirmingDelete ? onDelete() : setConfirmingDelete(true))}
            onBlur={() => setConfirmingDelete(false)}
          >
            {confirmingDelete ? "Click again to delete" : "Delete reminder"}
          </button>
        )}
        <span className="spacer" />
        <button type="button" className="button" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" className="button primary" disabled={saving}>
          {isNew ? "Add reminder" : "Save changes"}
        </button>
      </div>
    </form>
  );
}
