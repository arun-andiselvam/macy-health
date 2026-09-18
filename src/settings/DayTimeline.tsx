import type { Reminder } from "../types";
import { isAllDay, timesPerDay, toMinutes, windowMinutes } from "./format";

const DAY = 1440;
const pct = (minute: number) => `${(minute / DAY) * 100}%`;

/** A 24-hour track: the active window, with a tick for each expected popup. */
export function DayTimeline({ reminder }: { reminder: Reminder }) {
  const start = isAllDay(reminder) ? 0 : toMinutes(reminder.activeStart);
  const length = windowMinutes(reminder);
  const count = timesPerDay(reminder);

  // Active window, split in two when it crosses midnight.
  const segments: [number, number][] =
    start + length <= DAY ? [[start, length]] : [[start, DAY - start], [0, start + length - DAY]];

  const ticks = Array.from({ length: count }, (_, i) => (start + (i + 1) * reminder.intervalMinutes) % DAY);

  return (
    <div className="timeline">
      <div className="timeline-track" aria-hidden="true">
        {segments.map(([from, len]) => (
          <div key={from} className="timeline-window" style={{ left: pct(from), width: pct(len) }} />
        ))}
        {ticks.map((minute, i) => (
          <div key={i} className="timeline-tick" style={{ left: pct(minute) }} />
        ))}
        <div className="timeline-hours">
          {[0, 6, 12, 18, 24].map((h) => (
            <span key={h} style={{ left: pct(h * 60) }}>
              {String(h).padStart(2, "0")}
            </span>
          ))}
        </div>
      </div>
      <div className="timeline-count">
        {count === 0
          ? "Interval is longer than the active hours"
          : count === 1
            ? "Once a day"
            : `${count} times a day`}
      </div>
    </div>
  );
}
