import { describe, it, expect } from 'vitest';
import { firstCronOccurrence, isFutureLocal, nowInTimezone, todayInTimezone } from '../src/utils/schedule';

// 2026-10-15T20:00Z == 2026-10-16 01:30 IST
const NOW = new Date('2026-10-15T20:00:00Z');
const ALL = [0, 1, 2, 3, 4, 5, 6];

describe('schedule helpers', () => {
  it('uses the rule timezone for "today", not UTC', () => {
    expect(todayInTimezone('Asia/Kolkata', NOW)).toBe('2026-10-16');
    expect(todayInTimezone('UTC', NOW)).toBe('2026-10-15');
    expect(nowInTimezone('Asia/Kolkata', NOW).time).toBe('01:30');
  });

  it('first cron occurrence is today when the time has not passed', () => {
    const r = firstCronOccurrence({ timezone: 'Asia/Kolkata', daysOfWeek: ALL, runTimesLocal: ['03:00'], now: NOW });
    expect(r).toEqual({ date: '2026-10-16', time: '03:00', isToday: true });
  });

  it('skips a time that already passed today and rolls to tomorrow', () => {
    const r = firstCronOccurrence({ timezone: 'Asia/Kolkata', daysOfWeek: ALL, runTimesLocal: ['01:00'], now: NOW });
    expect(r).toEqual({ date: '2026-10-17', time: '01:00', isToday: false });
  });

  it('honours a later start date and day filter', () => {
    // 2026-10-20 is a Tuesday (2); only Mondays (1) -> next Monday 2026-10-26
    const r = firstCronOccurrence({ timezone: 'Asia/Kolkata', daysOfWeek: [1], runTimesLocal: ['09:00'], startDate: '2026-10-20', now: NOW });
    expect(r).toEqual({ date: '2026-10-26', time: '09:00', isToday: false });
  });

  it('returns null with no days selected', () => {
    expect(firstCronOccurrence({ timezone: 'UTC', daysOfWeek: [], runTimesLocal: ['09:00'], now: NOW })).toBeNull();
  });

  it('isFutureLocal compares in the given timezone', () => {
    expect(isFutureLocal('2026-10-16', '02:00', 'Asia/Kolkata', NOW)).toBe(true);
    expect(isFutureLocal('2026-10-16', '01:00', 'Asia/Kolkata', NOW)).toBe(false);
  });
});
