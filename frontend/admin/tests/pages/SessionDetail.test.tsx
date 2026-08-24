import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SessionDetail from '../../src/pages/SessionDetail';
import axios from 'axios';
import { MemoryRouter, Routes, Route } from 'react-router';

const BASE_SESSION = {
  _id: 'session1',
  isActive: true,
  expiresAt: new Date(Date.now() + 3600000).toISOString(),
  rotationCount: 0,
  locationName: 'Room 101',
  createdAt: new Date().toISOString(),
  monitoringEnabled: false,
};

const ATTENDANCE_RECORD = {
  _id: 'att1',
  rollNumber: 'ROLL001',
  studentName: 'Jane Student',
  photoUrl: 'https://example.com/photo.jpg',
  distanceFromLocation: 5,
  capturedAt: new Date().toISOString(),
  verified: true,
};

const BEHAVIOR_RESPONSE = {
  rollNumber: 'ROLL001',
  deviceLocks: [
    { id: 'lock1', extensionInstanceId: 'ext-instance-uuid', status: 'active', lockedAt: new Date().toISOString() },
  ],
  summary: {
    totalIdleSeconds: 120, focusTransitionCount: 3, tabSwitchCount: 5,
    meetTabEverOpen: true, eventCount: 8,
  },
  events: [
    { eventType: 'heartbeat', eventData: {}, recordedAt: new Date().toISOString() },
  ],
};

// jsdom has no WebSocket implementation — the monitoring dashboard opens one
// only while it's mounted (see useStudentBehavior's `live` param), so tests
// that open it need at least a no-op stand-in.
class MockWebSocket {
  onmessage: ((ev: unknown) => void) | null = null;
  close() {}
}

// Mirrors fetchData's Promise.all in SessionDetail.tsx — session/attendance/
// stats/absent/users, plus AdminSecurityReview's own calls (left to the
// default `{ data: {} }` mock from setupTests.js, which it tolerates fine).
const makeMockGet = (session: object = BASE_SESSION, attendanceRecords: object[] = []) => (url: string) => {
  if (url === '/api/admin/sessions/session1') return Promise.resolve({ data: session });
  if (url.includes('/behavior')) return Promise.resolve({ data: BEHAVIOR_RESPONSE });
  if (url.includes('/security-summary')) {
    return Promise.resolve({
      data: {
        totalSubmissions: 0, flaggedSubmissions: 0, flagPercentage: '0',
        unreviewedFlags: { gpsAnomalies: 0, emulatorDetected: 0, integrityIssues: 0 },
      },
    });
  }
  if (url.includes('/flagged')) return Promise.resolve({ data: [] });
  if (url.includes('/attendance')) return Promise.resolve({ data: attendanceRecords });
  if (url.includes('/stats')) return Promise.resolve({ data: { totalAttendance: 0, verifiedAttendance: 0, unverifiedAttendance: 0, rosterSize: 0, absentCount: 0, hasRoster: false } });
  if (url.includes('/absent')) return Promise.resolve({ data: [] });
  if (url.includes('/users')) return Promise.resolve({ data: [] });
  return Promise.resolve({ data: {} });
};

describe('SessionDetail', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('WebSocket', MockWebSocket);
  });

  const renderComponent = () => render(
    <MemoryRouter initialEntries={['/sessions/session1']}>
      <Routes>
        <Route path="/sessions/:id" element={<SessionDetail />} />
      </Routes>
    </MemoryRouter>
  );

  it('shows monitoring as Off by default', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    renderComponent();
    await waitFor(() => expect(screen.getByText('Off')).toBeInTheDocument());
  });

  it('shows monitoring as Enabled with its class duration when set', async () => {
    (axios.get as any).mockImplementation(makeMockGet({
      ...BASE_SESSION, monitoringEnabled: true, classDurationMinutes: 90,
    }));
    renderComponent();
    await waitFor(() => expect(screen.getByText(/Enabled/i)).toBeInTheDocument());
    expect(screen.getByText(/90 min class/i)).toBeInTheDocument();
  });

  it('opens the schedule edit modal and saves monitoring changes via PATCH .../schedule', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    (axios.patch as any).mockResolvedValue({ data: {} });
    renderComponent();
    await waitFor(() => expect(screen.getByText('Edit Schedule')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Edit Schedule'));
    expect(screen.getByText('Edit Session Schedule')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Enable monitoring'));
    const durationInput = screen.getByLabelText(/Full class duration/i);
    fireEvent.change(durationInput, { target: { value: '75' } });

    fireEvent.click(screen.getByText('Save Changes'));

    await waitFor(() => {
      expect(axios.patch).toHaveBeenCalledWith('/api/admin/sessions/session1/schedule', {
        monitoringEnabled: true,
        classDurationMinutes: 75,
      });
    });
  });

  it('disables the Edit Schedule button once the session started on a previous day', async () => {
    (axios.get as any).mockImplementation(makeMockGet({
      ...BASE_SESSION, createdAt: new Date(Date.now() - 86400000 * 2).toISOString(),
    }));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Edit Schedule')).toBeInTheDocument());
    expect(screen.getByText('Edit Schedule').closest('button')).toBeDisabled();
  });

  // ─── Student behavior viewer (Phase 4) ─────────────────────────────────

  it('hides the Monitoring button and per-row behavior action when monitoring is off', async () => {
    (axios.get as any).mockImplementation(makeMockGet(BASE_SESSION, [ATTENDANCE_RECORD]));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Jane Student')).toBeInTheDocument());
    expect(screen.queryByText('Monitoring')).not.toBeInTheDocument();

    fireEvent.click(screen.getByLabelText('Actions'));
    expect(screen.queryByText('View Behavior')).not.toBeInTheDocument();
  });

  it('opens the per-row behavior dialog and fetches the summary', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ ...BASE_SESSION, monitoringEnabled: true }, [ATTENDANCE_RECORD]));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Jane Student')).toBeInTheDocument());

    fireEvent.click(screen.getByLabelText('Actions'));
    fireEvent.click(screen.getByText('View Behavior'));

    await waitFor(() => {
      expect(axios.get).toHaveBeenCalledWith('/api/admin/sessions/session1/students/ROLL001/behavior');
    });
    expect(await screen.findByText('8 events')).toBeInTheDocument();
    expect(screen.getByText('Meet tab opened')).toBeInTheDocument();
  });

  it('opens the top-panel monitoring dashboard with a student picker', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ ...BASE_SESSION, monitoringEnabled: true }, [ATTENDANCE_RECORD]));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Monitoring')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Monitoring'));

    await waitFor(() => {
      expect(axios.get).toHaveBeenCalledWith('/api/admin/sessions/session1/students/ROLL001/behavior');
    });
    expect(screen.getByText('Jane Student (ROLL001)')).toBeInTheDocument();
    expect(await screen.findByText('8 events')).toBeInTheDocument();
  });
});
