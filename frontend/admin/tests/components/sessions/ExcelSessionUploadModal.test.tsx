import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import axios from 'axios';
import { toast } from 'react-toastify';
import ExcelSessionUploadModal from '../../../src/components/sessions/ExcelSessionUploadModal';
import type { Mentor } from '../../../src/pages/Sessions';

const MENTOR: Mentor = { _id: 'mentor1', username: 'jane.mentor', fullName: 'Jane Mentor', role: 'admin', isActive: true, email: 'jane@example.com' };

const PARSE_RESPONSE = {
  sourceFilename: 'roster.csv',
  sessionDate: '2026-08-20',
  totalRows: 3,
  totalGroups: 2,
  parseErrors: [],
  groups: [
    {
      groupIndex: 0,
      track: 'DSA',
      classLabel: 'CLS 1',
      sessionTimeRaw: '2:00 PM - 4:00 PM',
      startTime: '14:00',
      endTime: '16:00',
      durationMinutes: 120,
      collegeName: 'Test College',
      mentors: [{ email: 'jane@example.com', matchedAdminId: 'mentor1', matchedName: 'Jane Mentor' }],
      unmatchedMentorEmails: [],
      mentorEmailConflict: false,
      studentCount: 2,
      students: [
        { name: 'Alice', rollNumber: 'R1', email: 'alice@example.com', round: 'Python' },
        { name: 'Bob', rollNumber: 'R2', email: 'bob@example.com', round: 'Java' },
      ],
      issues: [],
    },
    {
      groupIndex: 1,
      track: 'Cybersecurity',
      classLabel: 'LH 2',
      sessionTimeRaw: '4:00 PM - 6:00 PM',
      startTime: '16:00',
      endTime: '18:00',
      durationMinutes: 120,
      collegeName: 'Test College',
      mentors: [],
      unmatchedMentorEmails: ['unknown@example.com'],
      mentorEmailConflict: false,
      studentCount: 1,
      students: [{ name: 'Carol', rollNumber: 'R3', email: 'carol@example.com' }],
      issues: [],
    },
  ],
};

const COMMIT_RESPONSE = {
  results: [{ groupIndex: 0, success: true, sessionId: 'session-1', excelBatchId: 'batch-1', studentsImported: 2 }],
  createdCount: 1,
  failedCount: 0,
};

describe('ExcelSessionUploadModal', () => {
  const onClose = vi.fn();
  const onMentorCreated = vi.fn();
  const onSessionsCreated = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  const renderComponent = (open = true) => render(
    <ExcelSessionUploadModal
      open={open}
      onClose={onClose}
      mentors={[MENTOR]}
      onMentorCreated={onMentorCreated}
      onSessionsCreated={onSessionsCreated}
    />
  );

  const selectFile = (container: HTMLElement, name = 'roster.csv', type = 'text/csv') => {
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File(['student_name,registration_number\nAlice,R1'], name, { type });
    fireEvent.change(input, { target: { files: [file] } });
    return input;
  };

  it('renders nothing when closed', () => {
    const { container } = renderComponent(false);
    expect(container).toBeEmptyDOMElement();
  });

  it('shows the upload stage with a disabled Parse button until a file is chosen', () => {
    const { container } = renderComponent();
    expect(screen.getByText(/Upload Roster/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Parse File/i })).toBeDisabled();

    selectFile(container);
    expect(screen.getByRole('button', { name: /Parse File/i })).not.toBeDisabled();
    expect(screen.getByText('roster.csv')).toBeInTheDocument();
  });

  it('rejects a file with an unsupported extension', () => {
    const { container } = renderComponent();
    selectFile(container, 'roster.pdf', 'application/pdf');
    expect(toast.error).toHaveBeenCalledWith(expect.stringMatching(/valid \.csv/i));
    expect(screen.queryByText('roster.pdf')).not.toBeInTheDocument();
  });

  it('parses the file and shows one tab per detected group', async () => {
    (axios.post as any).mockImplementation((url: string) => {
      if (url.includes('/excel-sessions/parse')) return Promise.resolve({ data: PARSE_RESPONSE });
      return Promise.resolve({ data: {} });
    });

    const { container } = renderComponent();
    selectFile(container);
    fireEvent.click(screen.getByRole('button', { name: /Parse File/i }));

    await waitFor(() => expect(screen.getByText(/Review 2 Detected Sessions/i)).toBeInTheDocument());
    expect(screen.getByText(/CLS 1 · 2:00 PM - 4:00 PM/)).toBeInTheDocument();
    expect(screen.getByText(/LH 2 · 4:00 PM - 6:00 PM/)).toBeInTheDocument();
    expect(screen.getAllByText('DSA').length).toBeGreaterThan(0);

    // Unmatched mentor email for the second group is surfaced with a
    // create-account affordance.
    fireEvent.click(screen.getByText(/LH 2 · 4:00 PM - 6:00 PM/));
    expect(screen.getByText(/unknown@example.com/)).toBeInTheDocument();
  });

  it('commits the active group with its prefilled matched mentor and reports back', async () => {
    (axios.post as any).mockImplementation((url: string) => {
      if (url.includes('/excel-sessions/parse')) return Promise.resolve({ data: PARSE_RESPONSE });
      if (url.includes('/excel-sessions/commit')) return Promise.resolve({ data: COMMIT_RESPONSE });
      return Promise.resolve({ data: {} });
    });

    const { container } = renderComponent();
    selectFile(container);
    fireEvent.change(screen.getByLabelText(/Session Date/i), { target: { value: '2026-08-20' } });
    fireEvent.click(screen.getByRole('button', { name: /Parse File/i }));
    await waitFor(() => expect(screen.getByText(/Review 2 Detected Sessions/i)).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Create This Session/i }));

    await waitFor(() => expect(onSessionsCreated).toHaveBeenCalled());

    const commitCall = (axios.post as any).mock.calls.find(
      ([url]: [string]) => url === '/api/admin/excel-sessions/commit'
    );
    expect(commitCall).toBeTruthy();
    const [, payload] = commitCall;
    expect(payload.sessionDate).toBe('2026-08-20');
    expect(payload.groups).toHaveLength(1);
    expect(payload.groups[0]).toMatchObject({
      groupIndex: 0,
      track: 'DSA',
      classLabel: 'CLS 1',
      assignedAdminIds: ['mentor1'],
      durationMinutes: 120,
    });
    expect(toast.success).toHaveBeenCalled();
  });

  it('blocks committing a group with no assigned mentor and surfaces an error', async () => {
    (axios.post as any).mockImplementation((url: string) => {
      if (url.includes('/excel-sessions/parse')) return Promise.resolve({ data: PARSE_RESPONSE });
      return Promise.resolve({ data: {} });
    });

    const { container } = renderComponent();
    selectFile(container);
    fireEvent.click(screen.getByRole('button', { name: /Parse File/i }));
    await waitFor(() => expect(screen.getByText(/Review 2 Detected Sessions/i)).toBeInTheDocument());

    // Switch to the second (unmatched-mentor) group and try to create it.
    fireEvent.click(screen.getByText(/LH 2 · 4:00 PM - 6:00 PM/));
    fireEvent.click(screen.getByRole('button', { name: /Create This Session/i }));

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith(expect.stringMatching(/at least one mentor/i)));
    expect(axios.post).not.toHaveBeenCalledWith('/api/admin/excel-sessions/commit', expect.anything());
  });

  it('creates a missing mentor account inline and assigns them to the group', async () => {
    const newMentor = { _id: 'mentor2', username: 'unknown', email: 'unknown@example.com', fullName: '', role: 'admin', isActive: true };
    (axios.post as any).mockImplementation((url: string) => {
      if (url.includes('/excel-sessions/parse')) return Promise.resolve({ data: PARSE_RESPONSE });
      if (url.includes('/api/admin/users')) return Promise.resolve({ data: newMentor });
      return Promise.resolve({ data: {} });
    });

    const { container } = renderComponent();
    selectFile(container);
    fireEvent.click(screen.getByRole('button', { name: /Parse File/i }));
    await waitFor(() => expect(screen.getByText(/Review 2 Detected Sessions/i)).toBeInTheDocument());
    fireEvent.click(screen.getByText(/LH 2 · 4:00 PM - 6:00 PM/));

    fireEvent.click(screen.getByText(/unknown@example.com — \+ Create account/));
    await waitFor(() => expect(screen.getByText(/Create mentor account for unknown@example.com/i)).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText('Username'), { target: { value: 'unknown.mentor' } });
    fireEvent.change(screen.getByPlaceholderText(/Set a password/i), { target: { value: 'password123' } });
    fireEvent.click(screen.getByRole('button', { name: /Create Account/i }));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/users', expect.objectContaining({
        email: 'unknown@example.com',
        role: 'admin',
      }));
    });
    await waitFor(() => expect(onMentorCreated).toHaveBeenCalledWith(newMentor));
  });
});
