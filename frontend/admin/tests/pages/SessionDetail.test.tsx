import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SessionDetail from '../../src/pages/SessionDetail';
import axios from 'axios';
import { toast } from 'react-toastify';
import { MemoryRouter, Routes, Route } from 'react-router';

// The shared setup mocks `toast` as a plain object, but SessionDetail's bulk-verify
// undo flow calls `toast(...)` directly (for the custom undo toast body) — override
// it here, locally, to be callable while keeping the success/error/etc. methods.
vi.mock('react-toastify', () => {
  const toastFn = vi.fn();
  toastFn.success = vi.fn();
  toastFn.error = vi.fn();
  toastFn.info = vi.fn();
  toastFn.warning = vi.fn();
  toastFn.dismiss = vi.fn();
  return {
    toast: toastFn,
    ToastContainer: () => null,
  };
});

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

  // ─── Session actions: rotate / deactivate ──────────────────────────────

  it('rotates the token after confirming, copying the new link to the clipboard', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    (axios.post as any).mockResolvedValue({ data: { token: 'newtoken123' } });
    const writeText = vi.fn();
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Rotate Token')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Rotate Token'));
    expect(screen.getByText('Rotate token? The current link will stop working.')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Rotate'));

    await waitFor(() => expect(axios.post).toHaveBeenCalledWith('/api/admin/sessions/session1/rotate'));
    expect(toast.success).toHaveBeenCalledWith('Token rotated! New link copied.');
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining('newtoken123'));
  });

  it('cancels the rotate-token confirmation without calling the API', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    renderComponent();
    await waitFor(() => expect(screen.getByText('Rotate Token')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Rotate Token'));
    fireEvent.click(screen.getByText('Cancel'));

    expect(axios.post).not.toHaveBeenCalled();
    expect(screen.queryByText('Rotate token? The current link will stop working.')).not.toBeInTheDocument();
  });

  it('shows an error toast when rotating the token fails', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    (axios.post as any).mockRejectedValue(new Error('fail'));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Rotate Token')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Rotate Token'));
    fireEvent.click(screen.getByText('Rotate'));

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Failed to rotate token'));
  });

  it('deactivates an active session after confirming', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    (axios.post as any).mockResolvedValue({ data: {} });
    renderComponent();
    await waitFor(() => expect(screen.getByText('Deactivate Session')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Deactivate Session'));
    fireEvent.click(screen.getByText('Deactivate'));

    await waitFor(() => expect(axios.post).toHaveBeenCalledWith('/api/admin/sessions/session1/deactivate'));
    expect(toast.success).toHaveBeenCalledWith('Session deactivated');
  });

  it('does not render the Deactivate Session button once the session is inactive', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ ...BASE_SESSION, isActive: false }));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Inactive')).toBeInTheDocument());
    expect(screen.queryByText('Deactivate Session')).not.toBeInTheDocument();
  });

  // ─── Export ─────────────────────────────────────────────────────────────

  it('disables the Export Verified button when there are no verified records', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    renderComponent();
    await waitFor(() => expect(screen.getByText(/Export Verified/)).toBeInTheDocument());
    expect(screen.getByText(/Export Verified/).closest('button')).toBeDisabled();
  });

  it('exports verified attendance as a blob download', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/export')) return Promise.resolve({ data: 'binarydata' });
      return makeMockGet(BASE_SESSION, [ATTENDANCE_RECORD])(url);
    });
    renderComponent();
    await waitFor(() => expect(screen.getByText(/Export Verified \(1\)/)).toBeInTheDocument());

    fireEvent.click(screen.getByText(/Export Verified \(1\)/));

    await waitFor(() => expect(axios.get).toHaveBeenCalledWith(
      '/api/admin/sessions/session1/export',
      expect.objectContaining({ responseType: 'blob' })
    ));
    expect(toast.success).toHaveBeenCalledWith('Attendance exported!');
  });

  // ─── Tabs, selection, and per-row verification ─────────────────────────

  it('switches tabs between all / verified / unverified and shows correct counts', async () => {
    const verifiedRec = { ...ATTENDANCE_RECORD, _id: 'att1', verified: true };
    const unverifiedRec = { ...ATTENDANCE_RECORD, _id: 'att2', rollNumber: 'ROLL002', studentName: 'Bob Student', verified: false };
    (axios.get as any).mockImplementation(makeMockGet(BASE_SESSION, [verifiedRec, unverifiedRec]));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Jane Student')).toBeInTheDocument());
    expect(screen.getByText('Bob Student')).toBeInTheDocument();

    fireEvent.click(document.getElementById('tab-verified')!);
    expect(screen.getByText('Jane Student')).toBeInTheDocument();
    expect(screen.queryByText('Bob Student')).not.toBeInTheDocument();

    fireEvent.click(document.getElementById('tab-unverified')!);
    expect(screen.queryByText('Jane Student')).not.toBeInTheDocument();
    expect(screen.getByText('Bob Student')).toBeInTheDocument();

    fireEvent.click(document.getElementById('tab-all')!);
    expect(screen.getByText('Jane Student')).toBeInTheDocument();
    expect(screen.getByText('Bob Student')).toBeInTheDocument();
  });

  it('shows the Absent tab with roster students when a batch is attached', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/absent')) {
        return Promise.resolve({ data: [{ name: 'Missing Student', rollNumber: 'ROLL999', collegeName: 'MIT' }] });
      }
      return makeMockGet({ ...BASE_SESSION, batchId: { _id: 'b1', name: 'CS Batch' } })(url);
    });
    renderComponent();
    await waitFor(() => expect(document.getElementById('tab-absent')).toBeInTheDocument());

    fireEvent.click(document.getElementById('tab-absent')!);
    expect(await screen.findByText('Missing Student')).toBeInTheDocument();
    expect(screen.getByText('MIT')).toBeInTheDocument();
  });

  it('selects a row via checkbox, bulk-marks it verified, and clears the selection', async () => {
    const unverifiedRec = { ...ATTENDANCE_RECORD, verified: false };
    (axios.get as any).mockImplementation(makeMockGet(BASE_SESSION, [unverifiedRec]));
    (axios.post as any).mockResolvedValue({ data: {} });
    renderComponent();
    await waitFor(() => expect(screen.getByText('Jane Student')).toBeInTheDocument());

    fireEvent.click(document.getElementById(`check-${unverifiedRec._id}`)!);
    expect(screen.getByText('Mark All Verified')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Mark All Verified'));
    // Optimistic UI update happens immediately; the bulk bar disappears.
    expect(screen.queryByText('Mark All Verified')).not.toBeInTheDocument();
  });

  it('marks a single record as verified from the kebab menu', async () => {
    const unverifiedRec = { ...ATTENDANCE_RECORD, verified: false };
    (axios.get as any).mockImplementation(makeMockGet(BASE_SESSION, [unverifiedRec]));
    (axios.patch as any).mockResolvedValue({ data: {} });
    renderComponent();
    await waitFor(() => expect(screen.getByText('Jane Student')).toBeInTheDocument());

    fireEvent.click(screen.getByLabelText('Actions'));
    fireEvent.click(screen.getByText('Mark as Verified'));

    await waitFor(() => expect(axios.patch).toHaveBeenCalledWith(
      `/api/admin/attendance/${unverifiedRec._id}/verify`,
      { verified: true }
    ));
    expect(toast.success).toHaveBeenCalledWith('Jane Student marked verified');
  });

  it('rolls back and shows an error toast when single-record verification fails', async () => {
    const unverifiedRec = { ...ATTENDANCE_RECORD, verified: false };
    (axios.get as any).mockImplementation(makeMockGet(BASE_SESSION, [unverifiedRec]));
    (axios.patch as any).mockRejectedValue(new Error('fail'));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Jane Student')).toBeInTheDocument());

    fireEvent.click(screen.getByLabelText('Actions'));
    fireEvent.click(screen.getByText('Mark as Verified'));

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Failed to update verification status'));
  });

  it('shows "Session not found" when the session fails to load', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url === '/api/admin/sessions/session1') return Promise.reject(new Error('404'));
      return makeMockGet()(url);
    });
    renderComponent();
    await waitFor(() => expect(screen.getByText('Session not found')).toBeInTheDocument());
  });
});
