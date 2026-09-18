export type Reminder = {
  id: string;
  kind: string;
  emoji: string;
  title: string;
  message: string;
  intervalMinutes: number;
  enabled: boolean;
  sound: boolean;
  /** "HH:MM" local time. Equal start and end means all day. */
  activeStart: string;
  activeEnd: string;
  /** ISO weekdays, 1 = Monday … 7 = Sunday. */
  days: number[];
};

export type General = {
  launchAtLogin: boolean;
  /** 0 = off */
  idlePauseMinutes: number;
  popupSeconds: number;
  /** Epoch ms, or null when not paused. */
  pausedUntil: number | null;
};

export type UpdateStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "upToDate" }
  | { state: "available"; version: string }
  | { state: "installing" }
  | { state: "failed"; message: string };

export type UpdateInfo = {
  currentVersion: string;
  status: UpdateStatus;
};
