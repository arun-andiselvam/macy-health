import type { Reminder } from "../types";

const SHORT_DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
export const DAY_NAMES = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
export const DAY_INITIALS = ["M", "T", "W", "T", "F", "S", "S"];

export const toMinutes = (hhmm: string) => {
  const [h, m] = hhmm.split(":").map(Number);
  return h * 60 + m;
};

export const isAllDay = (r: Pick<Reminder, "activeStart" | "activeEnd">) =>
  r.activeStart === r.activeEnd;

/** Length of the active window in minutes (crossing midnight is allowed). */
export const windowMinutes = (r: Pick<Reminder, "activeStart" | "activeEnd">) =>
  isAllDay(r) ? 1440 : (toMinutes(r.activeEnd) - toMinutes(r.activeStart) + 1440) % 1440;

/** Popups that fit strictly inside the window, one interval apart after it opens. */
export const timesPerDay = (r: Reminder) => {
  if (r.intervalMinutes <= 0) return 0;
  // All day has no edge, so the interval simply divides the day.
  if (isAllDay(r)) return Math.floor(1440 / r.intervalMinutes);
  return Math.max(0, Math.ceil(windowMinutes(r) / r.intervalMinutes) - 1);
};

export function describeInterval(minutes: number) {
  if (minutes < 60) return `Every ${minutes} min`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (m === 0) return h === 1 ? "Every hour" : `Every ${h} hours`;
  return `Every ${h} h ${m} min`;
}

export function describeDays(days: number[]) {
  const set = [...new Set(days)].sort();
  const key = set.join("");
  if (key === "1234567") return "every day";
  if (key === "12345") return "on weekdays";
  if (key === "67") return "on weekends";
  return `on ${set.map((d) => SHORT_DAYS[d - 1]).join(", ")}`;
}

/** "Every 45 min between 08:00 and 20:00, every day" */
export function describeSchedule(r: Reminder) {
  const hours = isAllDay(r) ? "around the clock" : `between ${r.activeStart} and ${r.activeEnd}`;
  return `${describeInterval(r.intervalMinutes)} ${hours}, ${describeDays(r.days)}`;
}
