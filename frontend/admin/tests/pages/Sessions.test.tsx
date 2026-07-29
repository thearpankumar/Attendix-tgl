import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Sessions from '../../src/pages/Sessions';
import axios from 'axios';
import { MemoryRouter } from 'react-router';

// Shared mock factory — always returns correct shape for all three GET endpoints
const makeMockGet = ({
  sessions = [] as object[],
  locations = [] as object[],
  shortLinks = [] as object[],
  batches = [] as object[],
} = {}) =>
  (url: string) => {
    if (url.includes('/sessions')) return Promise.resolve({ data: sessions });
    if (url.includes('/locations')) return Promise.resolve({ data: locations });
    if (url.includes('/shortlinks')) return Promise.resolve({ data: { shortLinks } });
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

  // ─── Short Link modes ─────────────────────────────────────────────────────

  it('auto-generate mode: creates session + new shortlink', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION] }));
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
        shortlinkMode: 'auto',
      }));
    });
  });

  it('attach existing mode: shows dropdown of unassigned links and calls attach API', async () => {
    (axios.get as any).mockImplementation(makeMockGet({ locations: [LOCATION], shortLinks: [FREE_LINK] }));
    (axios.post as any).mockImplementation((url: string) => {
      if (url.includes('/sessions')) return Promise.resolve({ data: { _id: 'new-session-id', shortCode: 'cs101' } });
      return Promise.resolve({ data: {} });
    });

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    // Switch to "Attach existing" mode
    fireEvent.click(screen.getByText(/Attach existing/i));
    // Dropdown with link should appear
    await waitFor(() => expect(screen.getByText(/\/s\/cs101/)).toBeInTheDocument());

    // Select location (by label to avoid ambiguity with the links dropdown)
    fireEvent.change(screen.getByLabelText(/^Location$/i), { target: { value: 'loc1' } });
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
    }));

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Session')[0]);
    fireEvent.click(screen.getByText(/Attach existing/i));

    await waitFor(() => expect(screen.getByText(/\/s\/cs101 \(In use\)/)).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText(/^Location$/i), { target: { value: 'loc1' } });
    const linkSelect = screen.getByDisplayValue('Pick a short link…');
    fireEvent.change(linkSelect, { target: { value: 'cs101' } });
    
    // Submit should open the modal, not call the API immediately
    fireEvent.submit(container.querySelector('form')!);
    
    await waitFor(() => expect(screen.getByText(/Short Link In Use/i)).toBeInTheDocument());
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
});
