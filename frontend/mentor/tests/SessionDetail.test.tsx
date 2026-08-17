import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { MemoryRouter, Routes, Route } from 'react-router';
import axios from 'axios';
import SessionDetail from '../src/pages/SessionDetail';

const ROSTER_ONE_UNMARKED = {
  session: { _id: 'sess1', collegeName: 'XYZ College', batchName: 'CS101', expiresAt: new Date().toISOString() },
  students: [
    { studentId: 's1', rollNumber: '21CS001', name: 'Asha Rao', status: 'unmarked', source: null, markedAt: null },
  ],
  summary: { total: 1, marked: 0, present: 0, absent: 0, unmarked: 1 },
};

const ROSTER_ALL_MARKED = {
  session: { _id: 'sess1', collegeName: 'XYZ College', batchName: 'CS101', expiresAt: new Date().toISOString() },
  students: [
    { studentId: 's1', rollNumber: '21CS001', name: 'Asha Rao', status: 'present', source: 'manual', markedAt: new Date().toISOString() },
  ],
  summary: { total: 1, marked: 1, present: 1, absent: 0, unmarked: 0 },
};

const renderSessionDetail = () => render(
  <MemoryRouter initialEntries={['/sessions/sess1']}>
    <Routes>
      <Route path="/sessions/:id" element={<SessionDetail />} />
    </Routes>
  </MemoryRouter>
);

const ROSTER_NOT_STARTED = {
  session: {
    _id: 'sess1',
    collegeName: 'XYZ College',
    batchName: 'CS101',
    startsAt: new Date(Date.now() + 3_600_000).toISOString(),
    expiresAt: new Date(Date.now() + 7_200_000).toISOString(),
  },
  students: [
    { studentId: 's1', rollNumber: '21CS001', name: 'Asha Rao', status: 'unmarked', source: null, markedAt: null },
  ],
  summary: { total: 1, marked: 0, present: 0, absent: 0, unmarked: 1 },
};

describe('Mentor SessionDetail (roster + manual marking)', () => {
  beforeEach(() => {
    vi.mocked(axios.get).mockReset();
    vi.mocked(axios.post).mockReset();
    vi.mocked(axios.delete).mockReset();
  });

  it('shows a "starts soon" state instead of the marking UI for a session that has not started', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_NOT_STARTED });
    renderSessionDetail();

    await waitFor(() => expect(screen.getByText(/hasn't started yet/i)).toBeInTheDocument());

    expect(screen.queryByText('Asha Rao')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /present/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /absent/i })).not.toBeInTheDocument();
  });

  it('shows the unmarked student and progress summary', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_ONE_UNMARKED });
    renderSessionDetail();

    await waitFor(() => {
      expect(screen.getByText('Asha Rao')).toBeInTheDocument();
      expect(screen.getByText('21CS001')).toBeInTheDocument();
      expect(screen.getByText('0/1 marked')).toBeInTheDocument();
    });
  });

  it('marking present calls the manual-mark endpoint with the right body', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_ONE_UNMARKED });
    vi.mocked(axios.post).mockResolvedValue({ data: { success: true, attendanceId: 'a1', status: 'present', rollNumber: '21CS001' } });
    renderSessionDetail();

    await waitFor(() => expect(screen.getByText('Asha Rao')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /present/i }));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith(
        '/api/admin/sessions/sess1/attendance/manual',
        { rollNumber: '21CS001', status: 'present' }
      );
    });
  });

  it('marking absent calls the manual-mark endpoint with status absent', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_ONE_UNMARKED });
    vi.mocked(axios.post).mockResolvedValue({ data: { success: true, attendanceId: 'a1', status: 'absent', rollNumber: '21CS001' } });
    renderSessionDetail();

    await waitFor(() => expect(screen.getByText('Asha Rao')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /absent/i }));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith(
        '/api/admin/sessions/sess1/attendance/manual',
        { rollNumber: '21CS001', status: 'absent' }
      );
    });
  });

  it('shows the all-marked celebration state once everyone has a status', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_ALL_MARKED });
    renderSessionDetail();

    await waitFor(() => {
      expect(screen.getByText(/all students marked/i)).toBeInTheDocument();
      expect(screen.getByText('1/1 marked')).toBeInTheDocument();
    });
  });

  it('undo calls the delete endpoint for the already-marked student', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_ALL_MARKED });
    vi.mocked(axios.delete).mockResolvedValue({ data: { success: true } });
    renderSessionDetail();

    await waitFor(() => expect(screen.getByText('Asha Rao')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /undo/i }));

    await waitFor(() => {
      expect(axios.delete).toHaveBeenCalledWith('/api/admin/sessions/sess1/attendance/manual/21CS001');
    });
  });

  // ─── List view ────────────────────────────────────────────────────────

  const ROSTER_MIXED = {
    session: { _id: 'sess1', collegeName: 'XYZ College', batchName: 'CS101', expiresAt: new Date().toISOString() },
    students: [
      { studentId: 's1', rollNumber: '21CS001', name: 'Asha Rao', status: 'unmarked', source: null, markedAt: null },
      { studentId: 's2', rollNumber: '21CS002', name: 'Bilal Khan', status: 'present', source: 'manual', markedAt: new Date().toISOString() },
      { studentId: 's3', rollNumber: '21CS003', name: 'Chen Wei', status: 'present', source: 'self_submitted', markedAt: new Date().toISOString() },
    ],
    summary: { total: 3, marked: 2, present: 2, absent: 0, unmarked: 1 },
  };

  it('switches to list view and shows every student regardless of status', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_MIXED });
    renderSessionDetail();

    await waitFor(() => expect(screen.getByText('Asha Rao')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /list/i }));

    // Unlike stack view, list view shows unmarked AND marked students together.
    expect(screen.getByText('Asha Rao')).toBeInTheDocument();
    expect(screen.getByText('Bilal Khan')).toBeInTheDocument();
    expect(screen.getByText('Chen Wei')).toBeInTheDocument();
  });

  it('list view: search filters students by name or roll number', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_MIXED });
    renderSessionDetail();
    await waitFor(() => expect(screen.getByText('Asha Rao')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /list/i }));

    fireEvent.change(screen.getByLabelText(/search students/i), { target: { value: '21cs002' } });

    expect(screen.getByText('Bilal Khan')).toBeInTheDocument();
    expect(screen.queryByText('Asha Rao')).not.toBeInTheDocument();
    expect(screen.queryByText('Chen Wei')).not.toBeInTheDocument();
  });

  it('list view: a self-submitted row has no undo button and is not draggable', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_MIXED });
    const { container } = renderSessionDetail();
    await waitFor(() => expect(screen.getByText('Asha Rao')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /list/i }));

    expect(screen.getByText(/Chen Wei/).closest('.list-row')).not.toBeNull();
    expect(screen.queryByLabelText(/undo mark for chen wei/i)).not.toBeInTheDocument();

    const rows = container.querySelectorAll('.list-row');
    const readOnlyRow = Array.from(rows).find((r) => r.textContent?.includes('Chen Wei'));
    expect(readOnlyRow?.className).toContain('list-row-readonly');
    expect(readOnlyRow?.getAttribute('drag')).toBeNull();
  });

  it('list view: a manually-marked row can be undone', async () => {
    vi.mocked(axios.get).mockResolvedValue({ data: ROSTER_MIXED });
    vi.mocked(axios.delete).mockResolvedValue({ data: { success: true } });
    renderSessionDetail();
    await waitFor(() => expect(screen.getByText('Asha Rao')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /list/i }));

    fireEvent.click(screen.getByLabelText(/undo mark for bilal khan/i));

    await waitFor(() => {
      expect(axios.delete).toHaveBeenCalledWith('/api/admin/sessions/sess1/attendance/manual/21CS002');
    });
  });
});
