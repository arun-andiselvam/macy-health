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
