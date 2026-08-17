import { Session } from '../api/types';
import { sessionState } from './useDashboard';

const baseSession: Session = {
  _id: 's1',
  isActive: true,
  expiresAt: '2026-08-17T12:00:00Z',
  attendanceCount: 0,
};

describe('sessionState', () => {
  const NOW = new Date('2026-08-17T10:00:00Z');

  afterEach(() => {
    jest.useRealTimers();
  });

  beforeEach(() => {
    jest.useFakeTimers().setSystemTime(NOW);
  });

  it('is closed when isActive is false, regardless of expiry', () => {
    const session: Session = { ...baseSession, isActive: false, expiresAt: '2026-08-17T23:00:00Z' };
    expect(sessionState(session)).toBe('closed');
  });

  it('is closed once expiresAt is in the past', () => {
    const session: Session = { ...baseSession, expiresAt: '2026-08-17T09:00:00Z' };
    expect(sessionState(session)).toBe('closed');
  });

  it('is upcoming when startsAt is in the future and the session has not expired', () => {
    const session: Session = { ...baseSession, expiresAt: '2026-08-17T23:00:00Z', startsAt: '2026-08-17T11:00:00Z' };
    expect(sessionState(session)).toBe('upcoming');
  });

  it('is live when there is no startsAt and the session has not expired', () => {
    const session: Session = { ...baseSession, expiresAt: '2026-08-17T23:00:00Z' };
    expect(sessionState(session)).toBe('live');
  });

  it('is live once startsAt has already passed and the session has not expired', () => {
    const session: Session = { ...baseSession, expiresAt: '2026-08-17T23:00:00Z', startsAt: '2026-08-17T09:00:00Z' };
    expect(sessionState(session)).toBe('live');
  });

  it('treats "closed" as taking priority over "upcoming"', () => {
    // isActive false AND startsAt in the future — closed must win.
    const session: Session = {
      ...baseSession,
      isActive: false,
      expiresAt: '2026-08-17T23:00:00Z',
      startsAt: '2026-08-17T11:00:00Z',
    };
    expect(sessionState(session)).toBe('closed');
  });
});
