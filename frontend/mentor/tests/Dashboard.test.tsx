import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { MemoryRouter } from 'react-router';
import axios from 'axios';
import Dashboard from '../src/pages/Dashboard';

vi.mock('../src/context/AuthContext', () => ({
  useAuth: () => ({ admin: { _id: 'm1', username: 'mentor.jane', fullName: 'Dr. Sarah', role: 'admin' } }),
}));

const ROSTER_SUMMARY = { total: 45, marked: 42, present: 40, absent: 2, unmarked: 3 };

// Fixed at noon so every fixture's Date.now()-relative offset (this file
// leans on "-1h" / "+1h" / "+1 day" to build live/today/tomorrow sessions)
// stays on the intended calendar day regardless of what time it actually is
// when the suite runs. Without this, a session built as "starts in 1h" —
// which Dashboard.tsx's isToday() must accept as still "today" — rolls into
// tomorrow whenever the suite happens to run within an hour of local
// midnight, which both fabricates and hides "Today" sessions depending on
// the wall clock. Vitest's fake Date is what Dashboard.tsx's own
// `new Date()`/`Date.now()` calls see too, so the component and the test
// fixtures always agree on "now".
const FIXED_NOW = new Date('2024-06-15T12:00:00');

// A function, not a module-level const: it must be evaluated after
// vi.setSystemTime() has pinned the clock (in beforeEach below), not once
// at import time against the real wall clock.
const liveSession = () => ({
  _id: 'sess1',
  collegeName: 'XYZ Engineering College',
  batchName: 'CS101',
  locationName: 'Room 101',
  startsAt: new Date(Date.now() - 3600_000).toISOString(),
  expiresAt: new Date(Date.now() + 3600_000).toISOString(),
  isActive: true,
  attendanceCount: 42,
});

const mockGet = (sessions: unknown[], roster: Record<string, unknown> = {}) => {
  (axios.get as any).mockImplementation((url: string) => {
    if (url === '/api/admin/sessions') return Promise.resolve({ data: sessions });
    const match = /\/api\/admin\/sessions\/([^/]+)\/roster/.exec(url);
    if (match) return Promise.resolve({ data: { summary: roster[match[1]] ?? ROSTER_SUMMARY } });
    return Promise.reject(new Error(`unexpected GET ${url}`));
  });
};

describe('Mentor Dashboard', () => {
  beforeEach(() => {
    vi.mocked(axios.get).mockReset();
    // shouldAdvanceTime: real time still elapses alongside the fake system
    // clock, so @testing-library/react's setTimeout-polling `waitFor` keeps
    // working without manual vi.advanceTimersByTime() calls in every test.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(FIXED_NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  const renderDashboard = () => render(
    <MemoryRouter>
      <Dashboard />
    </MemoryRouter>
  );

  it('shows a time-of-day greeting (no name — the Topbar already shows who is logged in) and the assigned session as a card', async () => {
    mockGet([liveSession()]);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText(/^Good (morning|afternoon|evening) 👋$/)).toBeInTheDocument();
      expect(screen.queryByText(/Dr\. Sarah/)).not.toBeInTheDocument();
      expect(screen.getByText('XYZ Engineering College · Room 101')).toBeInTheDocument();
      expect(screen.getAllByText('CS101').length).toBeGreaterThan(0);
      expect(screen.getByText('LIVE')).toBeInTheDocument();
    });
  });

  it('shows an empty state when the mentor has no assigned sessions', async () => {
    mockGet([]);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText(/no sessions assigned to you yet/i)).toBeInTheDocument();
    });
  });

  it('separates inactive sessions under a Past / Inactive heading', async () => {
    mockGet([{ ...liveSession(), _id: 'sess2', isActive: false, collegeName: 'Closed College' }]);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText(/past \/ inactive/i)).toBeInTheDocument();
      expect(screen.getByText('Closed College')).toBeInTheDocument();
    });
  });

  it('shows stat cards with computed active/upcoming/students/attendance figures', async () => {
    const upcoming = {
      ...liveSession(), _id: 'sess3', batchName: 'CS202',
      startsAt: new Date(Date.now() + 3_600_000).toISOString(),
    };
    mockGet([liveSession(), upcoming], { sess1: ROSTER_SUMMARY, sess3: { total: 10, marked: 0, present: 0, absent: 0, unmarked: 10 } });
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText('Your Sessions')).toBeInTheDocument();
      expect(screen.getByText('Active Sessions')).toBeInTheDocument();
      // "Upcoming" appears both as the stat-card label and (sans count, since
      // sess3 starts later today rather than a future day) the tab button.
      expect(screen.getAllByText('Upcoming').length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText('Students')).toBeInTheDocument();
      expect(screen.getByText('Attendance')).toBeInTheDocument();
      // 45 (sess1) + 10 (sess3) = 55 total students across open sessions
      expect(screen.getByText('55')).toBeInTheDocument();
    });
  });

  it('does not show a session starting tomorrow under "Today" — it belongs in the Upcoming tab', async () => {
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    tomorrow.setHours(9, 0, 0, 0);
    const tomorrowSession = {
      ...liveSession(), _id: 'sess-tomorrow', batchName: 'SRM IST', collegeName: 'SRM IST',
      startsAt: tomorrow.toISOString(),
      expiresAt: new Date(tomorrow.getTime() + 3_600_000).toISOString(),
    };
    mockGet([tomorrowSession]);
    renderDashboard();

    await waitFor(() => expect(screen.getByText('Your Sessions')).toBeInTheDocument());

    // Defaults to the "Today" tab — tomorrow's session must not appear here.
    expect(screen.queryByText('SRM IST')).not.toBeInTheDocument();
    expect(screen.getByText(/no sessions today/i)).toBeInTheDocument();

    // Switching to "Upcoming" reveals it.
    fireEvent.click(screen.getByRole('button', { name: /^Upcoming/ }));
    await waitFor(() => expect(screen.getAllByText('SRM IST').length).toBeGreaterThan(0));
  });

  it('does not show a misleading 100% Attendance when every session is upcoming, even if one has stray leftover marks', async () => {
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    const upcomingWithStrayMarks = {
      ...liveSession(), _id: 'sess-upcoming-marked', batchName: 'SRM IST',
      startsAt: tomorrow.toISOString(),
      expiresAt: new Date(tomorrow.getTime() + 3_600_000).toISOString(),
    };
    // One stray manual mark from before this session got rescheduled —
    // 1 present out of 9, nowhere near "100%".
    mockGet([upcomingWithStrayMarks], {
      'sess-upcoming-marked': { total: 9, marked: 1, present: 1, absent: 0, unmarked: 8 },
    });
    renderDashboard();

    await waitFor(() => expect(screen.getByText('Your Sessions')).toBeInTheDocument());

    expect(screen.getByText('—')).toBeInTheDocument();
    expect(screen.queryByText('100%')).not.toBeInTheDocument();
    expect(screen.queryByText('Attendance Overview')).not.toBeInTheDocument();
  });

  it('keeps a live session and one starting later today under the "Today" tab', async () => {
    const laterToday = new Date(Date.now() + 3_600_000);
    const todaySession = {
      ...liveSession(), _id: 'sess-later-today', batchName: 'CS303',
      startsAt: laterToday.toISOString(),
      expiresAt: new Date(laterToday.getTime() + 3_600_000).toISOString(),
    };
    mockGet([liveSession(), todaySession]);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getAllByText('CS101').length).toBeGreaterThan(0);
      expect(screen.getAllByText('CS303').length).toBeGreaterThan(0);
    });
  });

  it('renders an Attendance Overview donut once at least one student is marked', async () => {
    mockGet([liveSession()]);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText('Attendance Overview')).toBeInTheDocument();
      // 40 present / 42 marked = 95%, shown in both the stat card and the donut
      expect(screen.getAllByText('95%').length).toBe(2);
      expect(screen.getByText('40')).toBeInTheDocument();
      expect(screen.getByText('2')).toBeInTheDocument();
    });
  });
});
