import {
  addDaysISO,
  formatDate,
  formatDateForFilename,
  formatDateLong,
  formatDateTime,
  formatDayMonthTime,
  formatISODate,
  formatTime,
  formatTimeSeconds,
  formatWeekdayDateTime,
  istDateKey,
  istDayEndISO,
  istDayStartISO,
  istHour,
  istToUtcISO,
  todayIST,
} from '../../utils/datetime';

// Jest's sandbox does not honour a runtime TZ change, so the non-IST run is
// covered by the vitest suites (which do); run `TZ=America/Los_Angeles npx jest`
// to exercise this file under another zone too.

describe('datetime (IST only, independent of the device timezone)', () => {
  it('formats dates as "dd MMM yyyy" in IST', () => {
    expect(formatDate('2026-10-15T21:49:00Z')).toBe('16 Oct 2026');
    expect(formatDateLong('2026-10-15T21:49:00Z')).toBe('16 October 2026');
  });

  it('splits days exactly at IST midnight (18:30:00Z)', () => {
    expect(formatDate('2026-10-15T18:29:59Z')).toBe('15 Oct 2026');
    expect(formatDate('2026-10-15T18:30:00Z')).toBe('16 Oct 2026');
  });

  it('always uses the 3-letter month (Sep, never Sept) and no dots or commas', () => {
    const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
    months.forEach((m, i) => {
      const out = formatDate(`2026-${String(i + 1).padStart(2, '0')}-10T06:00:00Z`);
      expect(out).toBe(`10 ${m} 2026`);
      expect(out).not.toMatch(/[.,]/);
    });
    expect(formatDateLong('2026-09-10T06:00:00Z')).toBe('10 September 2026');
  });

  it('formats 12-hour times and date-times with the IST suffix', () => {
    expect(formatTime('2026-10-16T09:49:00Z')).toBe('03:19 PM');
    expect(formatTimeSeconds('2026-10-16T09:49:08Z')).toBe('03:19:08 PM');
    expect(formatDateTime('2026-10-16T09:49:00Z')).toBe('16 Oct 2026, 03:19 PM IST');
    expect(formatTime('2026-10-15T18:30:00Z')).toBe('12:00 AM');
    expect(formatTime('2026-10-16T06:30:00Z')).toBe('12:00 PM');
    expect(formatWeekdayDateTime('2026-10-16T09:49:00Z')).toBe('Fri, 16 Oct, 03:19 PM');
    expect(formatDayMonthTime('2026-10-16T09:49:00Z')).toBe('16 Oct, 03:19 PM');
  });

  it('returns an empty string for missing or invalid input', () => {
    expect(formatDate(undefined)).toBe('');
    expect(formatDateTime('not a date')).toBe('');
    expect(formatTime(null)).toBe('');
  });

  it("derives today's ISO date and hour in IST, not UTC or the device zone", () => {
    expect(todayIST(new Date('2026-10-15T18:29:59Z'))).toBe('2026-10-15');
    expect(todayIST(new Date('2026-10-15T18:30:00Z'))).toBe('2026-10-16');
    expect(istHour(new Date('2026-10-15T18:30:00Z'))).toBe(0);
    expect(istDateKey('2026-10-15T21:49:00Z')).toBe('2026-10-16');
  });

  it('converts IST days and wall-clock times to UTC instants', () => {
    expect(istDayStartISO('2026-10-16')).toBe('2026-10-15T18:30:00.000Z');
    expect(istDayEndISO('2026-10-16')).toBe('2026-10-16T18:29:59.999Z');
    expect(istToUtcISO('2026-10-16', '09:00')).toBe('2026-10-16T03:30:00.000Z');
  });

  it('does calendar math and bare-date formatting without timezone shifts', () => {
    expect(addDaysISO('2026-10-31', 1)).toBe('2026-11-01');
    expect(addDaysISO('2026-01-01', -1)).toBe('2025-12-31');
    expect(formatISODate('2026-10-16')).toBe('16 Oct 2026');
    expect(formatISODate('2026-09-05', true)).toBe('05 September 2026');
    expect(formatISODate('garbage')).toBe('');
    expect(formatDateForFilename('2026-10-15T21:49:00Z')).toBe('16-Oct-2026');
  });
});
