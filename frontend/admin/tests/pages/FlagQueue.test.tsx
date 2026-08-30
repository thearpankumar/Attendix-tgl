import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import FlagQueue from '../../src/pages/FlagQueue';
import axios from 'axios';

const RECORD = {
  _id: 'att1',
  sessionId: 'sess-11112222-3333-4444-5555-666677778888',
  rollNumber: 'CS101',
  studentName: 'Alice',
  capturedAt: new Date().toISOString(),
  flagged: true,
  flagReason: 'GPS anomalies detected',
  flagReviewed: false,
  flagSeverity: 'high',
  reviewNotes: null,
  gpsAnomalies: [{ type: 'POSITION_JUMP', severity: 'high', details: 'jumped 500km' }],
  emulatorDetected: false,
  emulatorFlags: [],
  integrityChecks: [],
};

const page = (items: object[], total = items.length) => ({
  data: { items, total, page: 1, pageSize: 25 },
});

describe('FlagQueue', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('fetches and displays flagged records from the unified queue endpoint', async () => {
    (axios.get as any).mockResolvedValue(page([RECORD]));

    render(<FlagQueue />);

    await waitFor(() => expect(screen.getByText('Alice')).toBeInTheDocument());
    expect(screen.getByText('CS101')).toBeInTheDocument();
    expect(screen.getByText('high')).toBeInTheDocument();
    expect(axios.get).toHaveBeenCalledWith(expect.stringContaining('/api/admin/security/flags/queue'));
  });

  it('shows an empty state when nothing is flagged', async () => {
    (axios.get as any).mockResolvedValue(page([]));

    render(<FlagQueue />);

    await waitFor(() => expect(screen.getByText('All clear')).toBeInTheDocument());
  });

  it('requires notes before submitting a single-row review, then posts them', async () => {
    (axios.get as any).mockResolvedValue(page([RECORD]));
    (axios.post as any).mockResolvedValue({ data: {} });

    render(<FlagQueue />);
    await waitFor(() => expect(screen.getByText('Alice')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Approve'));
    expect(await screen.findByText('Approve record')).toBeInTheDocument();

    const confirmButtons = screen.getAllByText('Confirm');
    const confirmButton = confirmButtons[confirmButtons.length - 1];
    expect(confirmButton).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText(/Verified with student/i), {
      target: { value: 'Confirmed with student, real GPS drift' },
    });
    expect(confirmButton).not.toBeDisabled();

    fireEvent.click(confirmButton);
    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/security/attendance/att1/review', {
        action: 'approve',
        notes: 'Confirmed with student, real GPS drift',
      });
    });
  });

  it('bulk-reviews selected rows via the bulk-review endpoint', async () => {
    const second = { ...RECORD, _id: 'att2', rollNumber: 'CS102', studentName: 'Bob' };
    (axios.get as any).mockResolvedValue(page([RECORD, second]));
    (axios.post as any).mockResolvedValue({ data: {} });

    render(<FlagQueue />);
    await waitFor(() => expect(screen.getByText('Alice')).toBeInTheDocument());

    const checkboxes = screen.getAllByRole('checkbox');
    // First checkbox is "select all"; row checkboxes follow.
    fireEvent.click(checkboxes[1]);
    fireEvent.click(checkboxes[2]);

    expect(screen.getByText('2 selected')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Reject selected'));
    expect(await screen.findByText('Reject 2 records')).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText(/Verified with student/i), {
      target: { value: 'Batch rejected, known bad device' },
    });
    fireEvent.click(screen.getByText('Confirm'));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/security/flags/bulk-review', {
        ids: ['att1', 'att2'],
        action: 'reject',
        notes: 'Batch rejected, known bad device',
      });
    });
  });
});
