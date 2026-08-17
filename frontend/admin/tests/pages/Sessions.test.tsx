import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Sessions from '../../src/pages/Sessions';
import axios from 'axios';
import { MemoryRouter } from 'react-router';

const EMPTY_OVERVIEW = {
  totalRecords: 0, filteredRecords: 0, present: 0, absent: 0, attendancePercent: 0,
  topSession: null, bottomSession: null,
};

// Shared mock factory — always returns correct shape for all GET endpoints
const makeMockGet = ({
  sessions = [] as object[],
  locations = [] as object[],
  shortLinks = [] as object[],
  batches = [] as object[],
  mentors = [BATCH_MENTOR] as object[],
  overview = EMPTY_OVERVIEW as object,
} = {}) =>
  (url: string) => {
    if (url.includes('/stats-overview')) return Promise.resolve({ data: overview });
    if (url.includes('/sessions')) return Promise.resolve({ data: sessions });
    if (url.includes('/locations')) return Promise.resolve({ data: locations });
    if (url.includes('/shortlinks')) return Promise.resolve({ data: { shortLinks } });
    if (url.includes('/users')) return Promise.resolve({ data: mentors });
    if (url.includes('/batches')) return Promise.resolve({ data: batches });
    return Promise.resolve({ data: [] });
  };

const ACTIVE_SESSION = {
  _id: '1', locationId: { name: 'Room 101' }, isActive: true,
  expiresAt: new Date(Date.now() + 10000).toISOString(),
  createdAt: new Date().toISOString(), attendanceCount: 0,
};
const LOCATION = { _id: 'loc1', name: 'Room 101', radiusMeters: 50 };
const FREE_LINK = { _id: 'sl1', shortCode: 'cs101', isActive: true, sessionId: null };
const BATCH = { _id: 'batch1', name: 'CS101', studentCount: 40 };
const BATCH_MENTOR = { _id: 'mentor1', username: 'mentor.jane', fullName: 'Jane Mentor', role: 'admin', isActive: true };

// Switches the create-session form to "Exam" mode and fills the fields that
// mode requires (batch, mentor, college name, start date/time) so tests can
// submit without the client-side exam-session validation short-circuiting
// them. Location is deliberately NOT filled — exam sessions don't have one.
// Normal-mode tests don't need this — batch stays optional there.
const fillRequiredSessionFields = () => {
  fireEvent.click(screen.getByText(/Exam \(assign a mentor\)/i));
  fireEvent.change(screen.getByLabelText(/^Batch$/i), { target: { value: 'batch1' } });
  // Mentor combobox: focus opens the dropdown, then click the option text.
  fireEvent.focus(screen.getByLabelText(/Assign Mentor/i));
  fireEvent.click(screen.getByText('Jane Mentor'));
  fireEvent.change(screen.getByLabelText(/^College Name$/i), { target: { value: 'XYZ College' } });
  fireEvent.change(screen.getByLabelText(/^Exam Date$/i), { target: { value: '2026-08-14' } });
  fireEvent.change(screen.getByLabelText(/^Start Time$/i), { target: { value: '09:00' } });
};

describe('Sessions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.assign(navigator, {
      clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
  });

  const renderComponent = () => render(
    <MemoryRouter>
      <Sessions />
    </MemoryRouter>
  );

  // ─── Rendering ────────────────────────────────────────────────────────────

  it('renders sessions list', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ sessions: [ACTIVE_SESSION], locations: [LOCATION] }));
    renderComponent();
    await waitFor(() => {
      expect(screen.getByText('Room 101')).toBeInTheDocument();
      expect(screen.getByText(/Active/i)).toBeInTheDocument();
    });
  });

  it('renders empty state when no sessions exist', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION] }));
    renderComponent();
    await waitFor(() => expect(screen.getByText(/No sessions yet/i)).toBeInTheDocument());
  });

  it('opens and closes the create session modal', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION] }));
    renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    expect(screen.getByText(/Select a location/i)).toBeInTheDocument();
    // Segmented selector should show all 3 modes
    expect(screen.getByText(/Auto-generate/i)).toBeInTheDocument();
    expect(screen.getByText(/Attach existing/i)).toBeInTheDocument();
    expect(screen.getByText(/Custom code/i)).toBeInTheDocument();

    fireEvent.click(screen.getByText(/Cancel/i));
    await waitFor(() => expect(screen.queryByText(/Select a location/i)).not.toBeInTheDocument());
  });

  // ─── Exam sessions ────────────────────────────────────────────────────────

  it('exam mode: creates session with mentor(s), batch, college and date/time, no location or shortlink', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION], batches: [BATCH] }));
    (axios.post as any).mockImplementation((url: string) => {
      if (url.includes('/sessions')) return Promise.resolve({ data: { _id: 'new-session-id' } });
      return Promise.resolve({ data: {} });
    });

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    fillRequiredSessionFields();
    fireEvent.submit(container.querySelector('form')!);

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/sessions', expect.objectContaining({
        locationId: undefined,
        batchId: 'batch1',
        assignedAdminIds: ['mentor1'],
        collegeName: 'XYZ College',
        startsAt: new Date('2026-08-14T09:00').toISOString(),
        shortlinkMode: 'none',
      }));
    });
    // Clipboard copy only happens when the backend returns a shortCode.
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
  });

  it('the Location field and Short Link section are hidden in exam mode', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION], batches: [BATCH] }));
    renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    fireEvent.click(screen.getByText(/Exam \(assign a mentor\)/i));

    expect(screen.queryByLabelText(/^Location$/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/^Short Link$/i)).not.toBeInTheDocument();
  });

  // ─── Short Link modes (normal sessions only) ─────────────────────────────

  it('auto-generate mode: creates session + new shortlink', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION], batches: [BATCH] }));
    (axios.post as any).mockImplementation((url: string) => {
      if (url.includes('/sessions')) return Promise.resolve({ data: { _id: 'new-session-id', shortCode: 'abc123' } });
      return Promise.resolve({ data: {} });
    });

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    fireEvent.change(screen.getByLabelText(/^Location$/i), { target: { value: 'loc1' } });
    // Default mode is 'auto' — submit without changing anything
    fireEvent.submit(container.querySelector('form')!);

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/sessions', expect.objectContaining({
        locationId: 'loc1',
        batchId: undefined,
        shortlinkMode: 'auto',
      }));
    });
  });

  it('attach existing mode: shows dropdown of unassigned links and calls attach API', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION], shortLinks: [FREE_LINK], batches: [BATCH] }));
    (axios.post as any).mockImplementation((url: string) => {
      if (url.includes('/sessions')) return Promise.resolve({ data: { _id: 'new-session-id', shortCode: 'cs101' } });
      return Promise.resolve({ data: {} });
    });

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    fireEvent.change(screen.getByLabelText(/^Location$/i), { target: { value: 'loc1' } });
    // Switch to "Attach existing" mode
    fireEvent.click(screen.getByText(/Attach existing/i));
    // Dropdown with link should appear
    await waitFor(() => expect(screen.getByText(/\/s\/cs101/)).toBeInTheDocument());

    // Pick the existing link
    const linkSelect = screen.getByDisplayValue('Pick a short link…');
    fireEvent.change(linkSelect, { target: { value: 'cs101' } });
    fireEvent.submit(container.querySelector('form')!);

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/sessions', expect.objectContaining({
        locationId: 'loc1',
        shortlinkMode: 'existing',
        existingShortCode: 'cs101',
      }));
    });
  });

  it('attach existing mode: shows warning when no active links are available', async () => {
    // No active links
    (axios.get as any).mockImplementation(makeMockGet({
      locations: [LOCATION],
      shortLinks: [],
    }));

    renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    fireEvent.click(screen.getByText(/Attach existing/i));

    await waitFor(() =>
      expect(screen.getByText(/No active short links available/i)).toBeInTheDocument()
    );
  });

  it('attach existing mode: shows reassignment modal when selecting an in-use link', async () => {
    (axios.get as any).mockImplementation(makeMockGet({
      locations: [LOCATION],
      shortLinks: [{ ...FREE_LINK, sessionId: 'some-other-session' }],
      batches: [BATCH],
    }));

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    fireEvent.change(screen.getByLabelText(/^Location$/i), { target: { value: 'loc1' } });
    fireEvent.click(screen.getByText(/Attach existing/i));

    await waitFor(() => expect(screen.getByText(/\/s\/cs101 \(In use\)/)).toBeInTheDocument());

    const linkSelect = screen.getByDisplayValue('Pick a short link…');
    fireEvent.change(linkSelect, { target: { value: 'cs101' } });

    // Submit should open the modal, not call the API immediately
    fireEvent.submit(container.querySelector('form')!);

    await waitFor(() => expect(screen.getByText(/Short Link In Use/i)).toBeInTheDocument());
  });

  // ─── Excel-upload bulk session creation ────────────────────────────────────

  it('opens the Excel upload modal', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION] }));
    renderComponent();
    await waitFor(() => expect(screen.getByText('Upload Excel')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Upload Excel'));
    expect(screen.getByText(/Upload Roster — Create Multiple Sessions/i)).toBeInTheDocument();
  });

  it('excludes Excel-generated session-batches from the manual batch dropdown', async () => {
    (axios.get as any).mockImplementation(makeMockGet({
      locations: [LOCATION],
      batches: [BATCH, { _id: 'sbatch1', name: 'CLS 1 — 2-4PM', studentCount: 12, type: 'session' }],
    }));
    renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    fireEvent.click(screen.getByText(/Exam \(assign a mentor\)/i));

    const batchSelect = screen.getByLabelText(/^Batch$/i);
    expect(screen.getByText('CS101 (40 students)')).toBeInTheDocument();
    expect(batchSelect.querySelector('option[value="sbatch1"]')).not.toBeInTheDocument();
  });

  // ─── Bulk export ────────────────────────────────────────────────────────────

  it('selects sessions via checkbox and exports them together', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ sessions: [ACTIVE_SESSION] }));
    (axios.post as any).mockResolvedValue({
      data: new Blob(['fake-xlsx']),
      headers: { 'content-disposition': 'attachment; filename="Attendance_Export_1sessions_17-Aug-2026_1200.xlsx"' },
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Room 101')).toBeInTheDocument());

    const rowCheckbox = screen.getByLabelText(/Select session 1/i);
    fireEvent.click(rowCheckbox);

    await waitFor(() => expect(screen.getByText(/Export Selected \(1\)/i)).toBeInTheDocument());
    fireEvent.click(screen.getByText(/Export Selected \(1\)/i));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith(
        '/api/admin/sessions/export-bulk',
        { sessionIds: ['1'] },
        expect.objectContaining({ responseType: 'blob' })
      );
    });
  });

  // ─── Delete ───────────────────────────────────────────────────────────────

  it('handles delete with password confirmation', async () => {
    (axios.get as any).mockImplementation(makeMockGet({
      sessions: [{ ...ACTIVE_SESSION, attendanceCount: 5 }],
    }));
    (axios.delete as any).mockResolvedValue({});

    renderComponent();
    await waitFor(() => expect(screen.getByText('Room 101')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Delete'));
    fireEvent.change(screen.getByPlaceholderText(/admin password/i), { target: { value: 'password123' } });
    fireEvent.click(screen.getByText('Confirm Delete'));

    await waitFor(() => {
      expect(axios.delete).toHaveBeenCalledWith('/api/admin/sessions/1', { data: { password: 'password123' } });
    });
  });

  // ─── Global stats + filters ─────────────────────────────────────────────

  it('renders the global stat cards from the stats-overview endpoint', async () => {
    (axios.get as any).mockImplementation(makeMockGet({
      sessions: [ACTIVE_SESSION],
      overview: {
        totalRecords: 20, filteredRecords: 12, present: 8, absent: 4, attendancePercent: 66.7,
        topSession: { id: '1', label: 'DSA — Room 3 — 4-6 PM', attendancePercent: 98 },
        bottomSession: { id: '2', label: 'Cyber — Room 7 — 2-4 PM', attendancePercent: 34 },
      },
    }));

    renderComponent();
    await waitFor(() => expect(screen.getByText('20')).toBeInTheDocument()); // total records
    expect(screen.getByText('12')).toBeInTheDocument(); // filtered records
    expect(screen.getByText('8')).toBeInTheDocument(); // present
    expect(screen.getByText('4')).toBeInTheDocument(); // absent
    expect(screen.getByText('67%')).toBeInTheDocument(); // rounded attendance %
    expect(screen.getByText(/Best attendance: DSA — Room 3 — 4-6 PM — 98%/i)).toBeInTheDocument();
    expect(screen.getByText(/Needs attention: Cyber — Room 7 — 2-4 PM — 34%/i)).toBeInTheDocument();
  });

  it('exports everything matching the active filters via "Export Filtered"', async () => {
    (axios.get as any).mockImplementation(makeMockGet({
      sessions: [ACTIVE_SESSION],
      overview: { ...EMPTY_OVERVIEW, filteredRecords: 3 },
    }));
    (axios.post as any).mockResolvedValue({
      data: new Blob(['fake-xlsx']),
      headers: { 'content-disposition': 'attachment; filename="Attendance_Export_Filtered.xlsx"' },
    });

    renderComponent();
    const exportButton = await screen.findByRole('button', { name: /Export Filtered \(3\)/i });
    fireEvent.click(exportButton);

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith(
        '/api/admin/sessions/export-filtered',
        expect.objectContaining({ batchIds: [], tracks: [], timeSlots: [] }),
        expect.objectContaining({ responseType: 'blob' })
      );
    });
  });

  it('loads and appends the next page of sessions when the scroll sentinel intersects', async () => {
    const makeSession = (i: number) => ({
      _id: `s${i}`, locationName: `Loc ${i}`, isActive: true,
      expiresAt: new Date(Date.now() + 10000).toISOString(),
      createdAt: new Date().toISOString(), attendanceCount: 0,
    });
    const page1 = Array.from({ length: 30 }, (_, i) => makeSession(i));
    const page2 = Array.from({ length: 5 }, (_, i) => makeSession(30 + i));

    (axios.get as any).mockImplementation((url: string, config?: { params?: { limit?: number; offset?: number } }) => {
      if (url.includes('/stats-overview')) return Promise.resolve({ data: EMPTY_OVERVIEW });
      if (url.includes('/sessions')) {
        const params = config?.params;
        // The unfiltered "all sessions" fetch (for filter-option derivation)
        // never passes `limit` — only the paginated display fetch does. Kept
        // empty here so its location names don't also show up as "All
        // Classes" dropdown options and collide with the table's own text.
        if (params?.limit === undefined) return Promise.resolve({ data: [] });
        return Promise.resolve({ data: params.offset === 0 ? page1 : page2 });
      }
      if (url.includes('/shortlinks')) return Promise.resolve({ data: { shortLinks: [] } });
      return Promise.resolve({ data: [] });
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Loc 0')).toBeInTheDocument());
    expect(screen.getByText('Loc 29')).toBeInTheDocument();
    expect(screen.queryByText('Loc 34')).not.toBeInTheDocument();

    // The sentinel mounts (and registers its IntersectionObserver) in an
    // effect that fires after the page-1 rows have already painted, so wait
    // for the instance itself rather than assuming it exists as soon as the
    // text does — otherwise this is flaky under CI's slower scheduling.
    await waitFor(() => expect(globalThis.__intersectionObserverInstances.length).toBeGreaterThan(0));
    const [observer] = globalThis.__intersectionObserverInstances.slice(-1);
    observer.callback([{ isIntersecting: true }]);

    await waitFor(() => expect(screen.getByText('Loc 34')).toBeInTheDocument());
    // Page-1 rows must still be there — appended, not replaced.
    expect(screen.getByText('Loc 0')).toBeInTheDocument();
  });

  it('deletes multiple selected sessions via the bulk-delete confirmation', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ sessions: [ACTIVE_SESSION] }));
    (axios.post as any).mockResolvedValue({ data: { success: true, deletedCount: 1, failedIds: [] } });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Room 101')).toBeInTheDocument());

    fireEvent.click(screen.getByLabelText(/Select session 1/i));
    await waitFor(() => expect(screen.getByText(/Delete Selected \(1\)/i)).toBeInTheDocument());
    fireEvent.click(screen.getByText(/Delete Selected \(1\)/i));

    fireEvent.change(screen.getByPlaceholderText(/admin password/i), { target: { value: 'password123' } });
    fireEvent.click(screen.getByText('Confirm Delete'));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/sessions/delete-bulk', {
        sessionIds: ['1'],
        password: 'password123',
      });
    });
  });
});
