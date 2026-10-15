/**
 * Wall-clock helpers for the session scheduling form. Everything is computed
 * in the rule's own IANA timezone (not the browser's), matching how the
 * backend interprets `runTimesLocal`/`startDate`/`runOnceDate`.
 */

const pad = (n: number) => String(n).padStart(2, '0');

/** "Now" in `timezone` as `{ date: 'YYYY-MM-DD', time: 'HH:MM' }`. */
export const nowInTimezone = (timezone: string, now: Date = new Date()): { date: string; time: string } => {
  let parts: Intl.DateTimeFormatPart[];
  try {
    parts = new Intl.DateTimeFormat('en-CA', {
      timeZone: timezone, hourCycle: 'h23',
      year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit',
    }).formatToParts(now);
  } catch {
    return nowInTimezone('UTC', now);
  }
  const get = (t: string) => parts.find((p) => p.type === t)?.value ?? '00';
  return { date: `${get('year')}-${get('month')}-${get('day')}`, time: `${get('hour')}:${get('minute')}` };
};

/** Today's date ('YYYY-MM-DD') in `timezone`. */
export const todayInTimezone = (timezone: string, now?: Date) => nowInTimezone(timezone, now).date;

const addDays = (date: string, days: number) => {
  const [y, m, d] = date.split('-').map(Number);
  const dt = new Date(Date.UTC(y, m - 1, d + days));
  return { date: `${dt.getUTCFullYear()}-${pad(dt.getUTCMonth() + 1)}-${pad(dt.getUTCDate())}`, weekday: dt.getUTCDay() };
};

/**
 * First local date/time a recurring rule will fire on, mirroring the
 * backend: days from `startDate` (default today) forward, a run time already
 * past today is skipped (no backfill). Returns null if nothing matches within
 * 8 days (e.g. no days selected).
 */
export const firstCronOccurrence = (opts: {
  timezone: string; daysOfWeek: number[]; runTimesLocal: string[]; startDate?: string; now?: Date;
}): { date: string; time: string; isToday: boolean } | null => {
  const now = nowInTimezone(opts.timezone, opts.now);
  const start = opts.startDate && opts.startDate > now.date ? opts.startDate : now.date;
  const times = [...opts.runTimesLocal].filter(Boolean).sort();
  for (let i = 0; i < 8; i++) {
    const { date, weekday } = addDays(start, i);
    if (!opts.daysOfWeek.includes(weekday)) continue;
    const t = times.find((x) => date > now.date || x > now.time);
    if (t) return { date, time: t, isToday: date === now.date };
  }
  return null;
};

/** True when the given local date+time in `timezone` is strictly in the future. */
export const isFutureLocal = (date: string, time: string, timezone: string, now?: Date) => {
  const n = nowInTimezone(timezone, now);
  return date > n.date || (date === n.date && time > n.time);
};
