/**
 * Date/time formatting for the whole product, which runs on IST only
 * (Asia/Kolkata, UTC+05:30, no DST). Everything here uses
 * `Intl.DateTimeFormat` with an explicit `timeZone: 'Asia/Kolkata'`, so the
 * output is the same whatever timezone the browser/device is set to.
 *
 * Display style: "16 Oct 2026" (date), "16 October 2026" (long form),
 * "03:19 PM" (12-hour time), "16 Oct 2026, 03:19 PM IST" (date-time).
 * Month names come from a fixed English table (built from `formatToParts`
 * numbers) so engines that print "Sept" or "Oct." can never leak through.
 *
 * Machine formats (API wire format, `<input type="date">` values) stay
 * ISO `YYYY-MM-DD` / RFC 3339; use `todayIST()` / `istDayBoundsISO()` for them.
 */

export const IST_TIMEZONE = 'Asia/Kolkata';

const MONTHS_SHORT = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const MONTHS_LONG = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

export type DateInput = string | number | Date | null | undefined;

const toDate = (input: DateInput): Date | null => {
  if (input === null || input === undefined || input === '') return null;
  const d = input instanceof Date ? input : new Date(input);
  return Number.isNaN(d.getTime()) ? null : d;
};

const partsFormatter = new Intl.DateTimeFormat('en-GB', {
  timeZone: IST_TIMEZONE,
  hourCycle: 'h23',
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
});

interface IstParts {
  year: number;
  month: number; // 1-12
  day: number;
  hour: number; // 0-23
  minute: number;
  second: number;
}

const istParts = (d: Date): IstParts => {
  const parts = partsFormatter.formatToParts(d);
  const val = (type: string) => Number(parts.find((p) => p.type === type)?.value ?? 0);
  return {
    year: val('year'),
    month: val('month'),
    day: val('day'),
    hour: val('hour') % 24,
    minute: val('minute'),
    second: val('second'),
  };
};

const pad2 = (n: number) => String(n).padStart(2, '0');

const timeString = (p: IstParts, withSeconds: boolean) => {
  const h12 = p.hour % 12 === 0 ? 12 : p.hour % 12;
  const suffix = p.hour >= 12 ? 'PM' : 'AM';
  return `${pad2(h12)}:${pad2(p.minute)}${withSeconds ? `:${pad2(p.second)}` : ''} ${suffix}`;
};

/** "16 Oct 2026" (IST). Empty string for a missing/invalid input. */
export const formatDate = (input: DateInput): string => {
  const d = toDate(input);
  if (!d) return '';
  const p = istParts(d);
  return `${pad2(p.day)} ${MONTHS_SHORT[p.month - 1]} ${p.year}`;
};

/** "16 October 2026" (IST), for headings and long-form text. */
export const formatDateLong = (input: DateInput): string => {
  const d = toDate(input);
  if (!d) return '';
  const p = istParts(d);
  return `${pad2(p.day)} ${MONTHS_LONG[p.month - 1]} ${p.year}`;
};

/** "03:19 PM" (12-hour, IST). */
export const formatTime = (input: DateInput): string => {
  const d = toDate(input);
  return d ? timeString(istParts(d), false) : '';
};

/** "03:19:08 PM" (12-hour with seconds, IST). */
export const formatTimeSeconds = (input: DateInput): string => {
  const d = toDate(input);
  return d ? timeString(istParts(d), true) : '';
};

/** "16 Oct 2026, 03:19 PM IST". */
export const formatDateTime = (input: DateInput): string => {
  const d = toDate(input);
  if (!d) return '';
  const p = istParts(d);
  return `${pad2(p.day)} ${MONTHS_SHORT[p.month - 1]} ${p.year}, ${timeString(p, false)} IST`;
};

/** "16 Oct, 03:19 PM" - compact date-time without the year, for tight cards. */
export const formatDayMonthTime = (input: DateInput): string => {
  const d = toDate(input);
  if (!d) return '';
  const p = istParts(d);
  return `${pad2(p.day)} ${MONTHS_SHORT[p.month - 1]}, ${timeString(p, false)}`;
};

/** The IST calendar date of an instant as ISO `YYYY-MM-DD` (input/wire format). */
export const istDateKey = (input: DateInput): string => {
  const d = toDate(input);
  if (!d) return '';
  const p = istParts(d);
  return `${p.year}-${pad2(p.month)}-${pad2(p.day)}`;
};

/** Today's date in IST as ISO `YYYY-MM-DD`, for date-input defaults/min/max. */
export const todayIST = (now: Date = new Date()): string => istDateKey(now);

/** Adds `days` to an ISO `YYYY-MM-DD` calendar date (timezone-free). */
export const addDaysISO = (iso: string, days: number): string => {
  const [y, m, d] = iso.split('-').map(Number);
  const dt = new Date(Date.UTC(y, m - 1, d + days));
  return `${dt.getUTCFullYear()}-${pad2(dt.getUTCMonth() + 1)}-${pad2(dt.getUTCDate())}`;
};

/** Formats a bare ISO `YYYY-MM-DD` calendar date (no tz shift): "16 Oct 2026". */
export const formatISODate = (iso: string, long = false): string => {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso ?? '');
  if (!m) return '';
  const month = Number(m[2]);
  if (month < 1 || month > 12) return '';
  return `${m[3]} ${(long ? MONTHS_LONG : MONTHS_SHORT)[month - 1]} ${m[1]}`;
};

/** Start of the IST day `YYYY-MM-DD` as a UTC RFC 3339 instant. */
export const istDayStartISO = (iso: string): string => new Date(`${iso}T00:00:00+05:30`).toISOString();

/** Last millisecond of the IST day `YYYY-MM-DD` as a UTC RFC 3339 instant. */
export const istDayEndISO = (iso: string): string =>
  new Date(new Date(`${addDaysISO(iso, 1)}T00:00:00+05:30`).getTime() - 1).toISOString();

/** An IST wall-clock `YYYY-MM-DD` + `HH:MM` as a UTC RFC 3339 instant. */
export const istToUtcISO = (date: string, time: string): string =>
  new Date(`${date}T${time.length === 5 ? `${time}:00` : time}+05:30`).toISOString();

/** File-name-safe date: "16-Oct-2026". */
export const formatDateForFilename = (input: DateInput): string => formatDate(input).replace(/ /g, '-');

/** Hour of day (0-23) in IST, for time-of-day greetings. */
export const istHour = (now: Date = new Date()): number => istParts(now).hour;

const WEEKDAYS_SHORT = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

/** "Fri, 16 Oct, 03:19 PM" - weekday + compact date-time (IST) for session cards. */
export const formatWeekdayDateTime = (input: DateInput): string => {
  const d = toDate(input);
  if (!d) return '';
  const p = istParts(d);
  const weekday = WEEKDAYS_SHORT[new Date(Date.UTC(p.year, p.month - 1, p.day)).getUTCDay()];
  return `${weekday}, ${pad2(p.day)} ${MONTHS_SHORT[p.month - 1]}, ${timeString(p, false)}`;
};
