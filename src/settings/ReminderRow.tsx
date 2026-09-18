import type { ReactNode } from "react";
import { Illustration } from "../popup/illustrations";
import type { Reminder } from "../types";
import { DayTimeline } from "./DayTimeline";
import { describeSchedule } from "./format";

type Props = {
  reminder: Reminder;
  expanded: boolean;
  onToggleExpanded: () => void;
  onToggleEnabled: (enabled: boolean) => void;
  children?: ReactNode;
};

export function ReminderRow({ reminder, expanded, onToggleExpanded, onToggleEnabled, children }: Props) {
  const isNew = reminder.id === "";
  const title = reminder.title.trim() || "New reminder";
  const panelId = `editor-${reminder.id || "new"}`;

  return (
    <li
      className={`row${reminder.enabled ? "" : " is-off"}${expanded ? " is-expanded" : ""}`}
      data-kind={reminder.kind}
    >
      <div className="row-main">
        <div className="row-icon" aria-hidden="true">
          <Illustration kind={reminder.kind} emoji={reminder.emoji} still />
        </div>

        <button
          type="button"
          className="row-summary"
          aria-expanded={expanded}
          aria-controls={panelId}
          onClick={onToggleExpanded}
          disabled={isNew}
        >
          <span className="row-title">{title}</span>
          <span className="row-schedule">{describeSchedule(reminder)}</span>
        </button>

        {!isNew && (
          <button
            type="button"
            role="switch"
            className="switch"
            aria-checked={reminder.enabled}
            aria-label={`${title} ${reminder.enabled ? "on" : "off"}`}
            onClick={() => onToggleEnabled(!reminder.enabled)}
          />
        )}

        <div className="row-timeline">
          <DayTimeline reminder={reminder} />
        </div>
      </div>

      {expanded && (
        <div id={panelId} className="row-editor">
          {children}
        </div>
      )}
    </li>
  );
}
