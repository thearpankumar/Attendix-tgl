import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { MemoryRouter } from 'react-router';
import axios from 'axios';
import Dashboard from '../src/pages/Dashboard';

vi.mock('../src/context/AuthContext', () => ({
  useAuth: () => ({ admin: { _id: 'm1', username: 'mentor.jane', fullName: 'Dr. Sarah', role: 'admin' } }),
}));

const ROSTER_SUMMARY = { total: 45, marked: 42, present: 40, absent: 2, unmarked: 3 };

const LIVE_SESSION = {
  _id: 'sess1',
  collegeName: 'XYZ Engineering College',
  batchName: 'CS101',
  locationName: 'Room 101',
  startsAt: new Date(Date.now() - 3600_000).toISOString(),
  expiresAt: new Date(Date.now() + 3600_000).toISOString(),
  isActive: true,
  attendanceCount: 42,
};

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
  });

  const renderDashboard = () => render(
    <MemoryRouter>
      <Dashboard />
    </MemoryRouter>
  );

  it('greets the mentor by name and shows the assigned session as a card', async () => {
    mockGet([LIVE_SESSION]);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText(/Dr\. Sarah/)).toBeInTheDocument();
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
    mockGet([{ ...LIVE_SESSION, _id: 'sess2', isActive: false, collegeName: 'Closed College' }]);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText(/past \/ inactive/i)).toBeInTheDocument();
      expect(screen.getByText('Closed College')).toBeInTheDocument();
    });
  });

  it('shows stat cards with computed active/upcoming/students/attendance figures', async () => {
    const upcoming = {
      ...LIVE_SESSION, _id: 'sess3', batchName: 'CS202',
      startsAt: new Date(Date.now() + 3_600_000).toISOString(),
    };
    mockGet([LIVE_SESSION, upcoming], { sess1: ROSTER_SUMMARY, sess3: { total: 10, marked: 0, present: 0, absent: 0, unmarked: 10 } });
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText('Your Sessions Today')).toBeInTheDocument();
      expect(screen.getByText('Active Sessions')).toBeInTheDocument();
      expect(screen.getByText('Upcoming')).toBeInTheDocument();
      expect(screen.getByText('Students')).toBeInTheDocument();
      expect(screen.getByText('Attendance')).toBeInTheDocument();
      // 45 (sess1) + 10 (sess3) = 55 total students across open sessions
      expect(screen.getByText('55')).toBeInTheDocument();
    });
  });

  it('renders an Attendance Overview donut once at least one student is marked', async () => {
    mockGet([LIVE_SESSION]);
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
