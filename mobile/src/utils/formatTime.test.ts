import { timeUntil } from './formatTime';

describe('timeUntil', () => {
  afterEach(() => {
    jest.useRealTimers();
  });

  it('returns "Starting now" once the start time has passed', () => {
    jest.useFakeTimers().setSystemTime(new Date('2026-08-17T10:00:00Z'));
    expect(timeUntil('2026-08-17T09:59:00Z')).toBe('Starting now');
  });

  it('returns "Starting now" exactly at the start time', () => {
    jest.useFakeTimers().setSystemTime(new Date('2026-08-17T10:00:00Z'));
    expect(timeUntil('2026-08-17T10:00:00Z')).toBe('Starting now');
  });

  it('formats a sub-hour countdown in minutes only', () => {
    jest.useFakeTimers().setSystemTime(new Date('2026-08-17T10:00:00Z'));
    expect(timeUntil('2026-08-17T10:25:00Z')).toBe('Starts in 25m');
  });

  it('formats a multi-hour countdown as hours and minutes', () => {
    jest.useFakeTimers().setSystemTime(new Date('2026-08-17T10:00:00Z'));
    expect(timeUntil('2026-08-17T12:30:00Z')).toBe('Starts in 2h 30m');
  });
});
