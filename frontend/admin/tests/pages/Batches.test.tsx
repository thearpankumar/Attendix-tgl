import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Batches from '../../src/pages/Batches';
import axios from 'axios';
import { toast } from 'react-toastify';
import { MemoryRouter } from 'react-router';

const mockNavigate = vi.fn();
vi.mock('react-router', async () => {
  const actual = await vi.importActual('react-router');
  return { ...actual, useNavigate: () => mockNavigate };
});

const makeBatch = (i: number) => ({
  _id: `b${i}`,
  name: `Batch ${i}`,
  description: '',
  studentCount: 5,
  createdAt: new Date(2026, 0, i + 1).toISOString(),
  type: 'manual' as const,
});

describe('Batches', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const renderComponent = () => render(
    <MemoryRouter>
      <Batches />
    </MemoryRouter>
  );

  it('renders the empty state when there are no batches', async () => {
    (axios.get as any).mockResolvedValue({ data: [] });
    renderComponent();
    await waitFor(() => expect(screen.getByText(/No Batches Found/i)).toBeInTheDocument());
  });

  it('loads the first page (limit=30, offset=0) on mount', async () => {
    const page1 = Array.from({ length: 30 }, (_, i) => makeBatch(i));
    (axios.get as any).mockImplementation((_url: string, config?: { params?: { offset?: number } }) => {
      const offset = config?.params?.offset ?? 0;
      return Promise.resolve({ data: offset === 0 ? page1 : [] });
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Batch 0')).toBeInTheDocument());
    expect(screen.getByText('Batch 29')).toBeInTheDocument();
    expect(axios.get).toHaveBeenCalledWith(
      '/api/admin/batches',
      expect.objectContaining({ params: { limit: 30, offset: 0 } })
    );
  });

  it('loads and appends the next page when the scroll sentinel intersects', async () => {
    const page1 = Array.from({ length: 30 }, (_, i) => makeBatch(i));
    const page2 = Array.from({ length: 5 }, (_, i) => makeBatch(30 + i));
    (axios.get as any).mockImplementation((_url: string, config?: { params?: { offset?: number } }) => {
      const offset = config?.params?.offset ?? 0;
      if (offset === 0) return Promise.resolve({ data: page1 });
      if (offset === 30) return Promise.resolve({ data: page2 });
      return Promise.resolve({ data: [] });
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Batch 0')).toBeInTheDocument());
    expect(screen.queryByText('Batch 34')).not.toBeInTheDocument();

    // The sentinel mounts (and registers its IntersectionObserver) in an
    // effect that fires after the page-1 rows have already painted, so wait
    // for the instance itself rather than assuming it exists as soon as the
    // text does — otherwise this is flaky under CI's slower scheduling.
    await waitFor(() => expect(globalThis.__intersectionObserverInstances.length).toBeGreaterThan(0));
    const [observer] = globalThis.__intersectionObserverInstances.slice(-1);
    observer.callback([{ isIntersecting: true }]);

    await waitFor(() => expect(screen.getByText('Batch 34')).toBeInTheDocument());
    // Original page-1 rows must still be there — appended, not replaced.
    expect(screen.getByText('Batch 0')).toBeInTheDocument();
  });

  it('does not render the scroll sentinel once every batch has been loaded', async () => {
    const fewBatches = Array.from({ length: 3 }, (_, i) => makeBatch(i));
    (axios.get as any).mockResolvedValue({ data: fewBatches });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Batch 0')).toBeInTheDocument());

    // A page shorter than the page size means there's nothing more to load.
    expect(globalThis.__intersectionObserverInstances.length).toBe(0);
  });

  it('shows an error toast when the initial batch fetch fails', async () => {
    (axios.get as any).mockRejectedValue(new Error('network error'));
    renderComponent();
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Failed to load batches'));
  });

  it('shows an error toast when fetching more batches fails', async () => {
    const page1 = Array.from({ length: 30 }, (_, i) => makeBatch(i));
    (axios.get as any).mockImplementation((_url: string, config?: { params?: { offset?: number } }) => {
      const offset = config?.params?.offset ?? 0;
      if (offset === 0) return Promise.resolve({ data: page1 });
      return Promise.reject(new Error('boom'));
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Batch 0')).toBeInTheDocument());
    await waitFor(() => expect(globalThis.__intersectionObserverInstances.length).toBeGreaterThan(0));
    const [observer] = globalThis.__intersectionObserverInstances.slice(-1);
    observer.callback([{ isIntersecting: true }]);

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Failed to load more batches'));
  });

  describe('create batch modal', () => {
    beforeEach(() => {
      (axios.get as any).mockResolvedValue({ data: [] });
    });

    const openModal = async () => {
      renderComponent();
      await waitFor(() => expect(screen.getByText(/No Batches Found/i)).toBeInTheDocument());
      fireEvent.click(screen.getByText('Import Your First Roster'));
      expect(screen.getByText('Import Student Batch')).toBeInTheDocument();
    };

    it('opens via the header Create Batch button and closes via the X button', async () => {
      renderComponent();
      await waitFor(() => expect(screen.getByText(/No Batches Found/i)).toBeInTheDocument());
      fireEvent.click(screen.getByText('Create Batch'));
      expect(screen.getByText('Import Student Batch')).toBeInTheDocument();

      const closeBtn = screen.getByText('Import Student Batch').closest('.modal-header')!.querySelector('.close-btn')!;
      fireEvent.click(closeBtn);
      expect(screen.queryByText('Import Student Batch')).not.toBeInTheDocument();
    });

    it('closes via the Cancel button', async () => {
      await openModal();
      fireEvent.click(screen.getByText('Cancel'));
      expect(screen.queryByText('Import Student Batch')).not.toBeInTheDocument();
    });

    it('rejects a file with an invalid type/extension', async () => {
      await openModal();
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
      const badFile = new File(['hello'], 'notes.txt', { type: 'text/plain' });
      fireEvent.change(fileInput, { target: { files: [badFile] } });

      expect(toast.error).toHaveBeenCalledWith('Please upload a valid .csv, .xlsx, .xls, or .ods file');
      expect(screen.queryByText('notes.txt')).not.toBeInTheDocument();
    });

    it('rejects a file larger than 10MB', async () => {
      await openModal();
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
      const bigFile = new File(['x'], 'roster.csv', { type: 'text/csv' });
      Object.defineProperty(bigFile, 'size', { value: 11 * 1024 * 1024 });
      fireEvent.change(fileInput, { target: { files: [bigFile] } });

      expect(toast.error).toHaveBeenCalledWith('File size must be less than 10MB');
    });

    it('accepts a valid file via the file input and shows its info', async () => {
      await openModal();
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
      const goodFile = new File(['a,b\n1,2'], 'roster.csv', { type: 'text/csv' });
      fireEvent.change(fileInput, { target: { files: [goodFile] } });

      expect(screen.getByText('roster.csv')).toBeInTheDocument();
      expect(screen.getByText('Click to change file')).toBeInTheDocument();
    });

    it('shows an error toast on submit with no file selected', async () => {
      await openModal();
      fireEvent.change(screen.getByLabelText('Batch Name'), { target: { value: 'CS 2024' } });
      fireEvent.submit(document.querySelector('.modal-body')!);
      expect(toast.error).toHaveBeenCalledWith('Please select a file to import');
    });

    it('submits the form and creates a batch successfully', async () => {
      (axios.post as any).mockResolvedValue({ data: {} });
      await openModal();

      fireEvent.change(screen.getByLabelText('Batch Name'), { target: { value: 'CS 2024' } });
      fireEvent.change(screen.getByLabelText(/Description/i), { target: { value: 'Morning batch' } });

      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
      const goodFile = new File(['a,b\n1,2'], 'roster.csv', { type: 'text/csv' });
      fireEvent.change(fileInput, { target: { files: [goodFile] } });

      fireEvent.click(screen.getByText('Create Batch', { selector: 'button[type="submit"]' }));

      await waitFor(() => expect(axios.post).toHaveBeenCalledWith(
        '/api/admin/batches',
        expect.any(FormData),
        expect.objectContaining({ headers: { 'Content-Type': 'multipart/form-data' } })
      ));
      expect(toast.success).toHaveBeenCalledWith('Batch created successfully!');
      await waitFor(() => expect(screen.queryByText('Import Student Batch')).not.toBeInTheDocument());
    });

    it('warns when the server skipped duplicate rows or flagged a repeated batch name', async () => {
      (axios.post as any).mockResolvedValue({
        data: { studentsImported: 2, studentsSkipped: 1, errors: ["Duplicate roll number 'R1' ('Alice Again') ignored; kept the first occurrence ('Alice')"] },
      });
      await openModal();

      fireEvent.change(screen.getByLabelText('Batch Name'), { target: { value: 'CS 2024' } });
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
      fireEvent.change(fileInput, { target: { files: [new File(['a,b\n1,2'], 'roster.csv', { type: 'text/csv' })] } });
      fireEvent.click(screen.getByText('Create Batch', { selector: 'button[type="submit"]' }));

      await waitFor(() => expect(toast.warning).toHaveBeenCalledWith(
        expect.stringContaining('2 students imported, 1 skipped'),
        expect.anything(),
      ));
    });

    it('shows the server-provided error message when batch creation fails', async () => {
      (axios.post as any).mockRejectedValue({ response: { data: { message: 'Duplicate batch name' } } });
      await openModal();

      fireEvent.change(screen.getByLabelText('Batch Name'), { target: { value: 'CS 2024' } });
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
      const goodFile = new File(['a,b\n1,2'], 'roster.csv', { type: 'text/csv' });
      fireEvent.change(fileInput, { target: { files: [goodFile] } });

      fireEvent.click(screen.getByText('Create Batch', { selector: 'button[type="submit"]' }));

      await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Duplicate batch name'));
      // Modal stays open on failure so the user can retry.
      expect(screen.getByText('Import Student Batch')).toBeInTheDocument();
    });

    it('falls back to a generic error message when the server gives none', async () => {
      (axios.post as any).mockRejectedValue(new Error('network down'));
      await openModal();

      fireEvent.change(screen.getByLabelText('Batch Name'), { target: { value: 'CS 2024' } });
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
      const goodFile = new File(['a,b\n1,2'], 'roster.csv', { type: 'text/csv' });
      fireEvent.change(fileInput, { target: { files: [goodFile] } });

      fireEvent.click(screen.getByText('Create Batch', { selector: 'button[type="submit"]' }));

      await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Failed to create batch'));
    });
  });

  describe('delete batch', () => {
    const mockGets = (impact: unknown) => {
      (axios.get as any).mockImplementation((url: string) =>
        Promise.resolve({ data: url.endsWith('/delete-impact') ? impact : [makeBatch(1)] })
      );
    };

    it('explains the consequence for a batch used by sessions, then deletes on confirm', async () => {
      mockGets({ sessionCount: 3, ruleCount: 1, activeRuleCount: 1, mode: 'archived' });
      (axios.delete as any).mockResolvedValue({ data: { mode: 'archived' } });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());

      fireEvent.click(screen.getByTitle('Delete Batch'));
      await waitFor(() => expect(screen.getByText('Delete batch?')).toBeInTheDocument());
      expect(axios.get).toHaveBeenCalledWith('/api/admin/batches/b1/delete-impact');
      expect(screen.getByText(/used by 3 sessions and 1 recurring schedule/)).toBeInTheDocument();
      expect(screen.getByText(/keep their roster, attendance, absent lists and exports/)).toBeInTheDocument();
      expect(screen.getByText(/will be paused/)).toBeInTheDocument();
      expect(axios.delete).not.toHaveBeenCalled();

      fireEvent.click(screen.getByText('Delete Batch', { selector: '.form-actions button' }));
      await waitFor(() => expect(axios.delete).toHaveBeenCalledWith('/api/admin/batches/b1'));
      expect(toast.success).toHaveBeenCalledWith('Batch deleted (past sessions and attendance kept)');
      await waitFor(() => expect(screen.queryByText('Batch 1')).not.toBeInTheDocument());
    });

    it('says an unreferenced batch is permanently deleted', async () => {
      mockGets({ sessionCount: 0, ruleCount: 0, activeRuleCount: 0, mode: 'deleted' });
      (axios.delete as any).mockResolvedValue({ data: { mode: 'deleted' } });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());
      fireEvent.click(screen.getByTitle('Delete Batch'));
      await waitFor(() => expect(screen.getByText(/permanently deleted/)).toBeInTheDocument());

      fireEvent.click(screen.getByText('Delete Batch', { selector: '.form-actions button' }));
      await waitFor(() => expect(toast.success).toHaveBeenCalledWith('Batch deleted'));
    });

    it('does not delete when the confirmation is dismissed', async () => {
      mockGets({ sessionCount: 1, ruleCount: 0, activeRuleCount: 0, mode: 'archived' });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());

      fireEvent.click(screen.getByTitle('Delete Batch'));
      await waitFor(() => expect(screen.getByText('Delete batch?')).toBeInTheDocument());
      fireEvent.click(screen.getByText('Cancel'));
      expect(axios.delete).not.toHaveBeenCalled();
      expect(screen.queryByText('Delete batch?')).not.toBeInTheDocument();
      expect(screen.getByText('Batch 1')).toBeInTheDocument();
    });

    it('shows the server message when delete fails', async () => {
      mockGets({ sessionCount: 0, ruleCount: 0, activeRuleCount: 0, mode: 'deleted' });
      (axios.delete as any).mockRejectedValue({ response: { data: { message: 'Batch not found' } } });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());

      fireEvent.click(screen.getByTitle('Delete Batch'));
      await waitFor(() => expect(screen.getByText('Delete batch?')).toBeInTheDocument());
      fireEvent.click(screen.getByText('Delete Batch', { selector: '.form-actions button' }));
      await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Batch not found'));
      expect(screen.getByText('Batch 1')).toBeInTheDocument();
    });

    it('does not render a delete button for session-type batches', async () => {
      const sessionBatch = { ...makeBatch(1), type: 'session' as const, linkedSessionId: 'sess1' };
      (axios.get as any).mockResolvedValue({ data: [sessionBatch] });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());
      expect(screen.queryByTitle('Delete Batch')).not.toBeInTheDocument();
      expect(screen.getByText('Session Batch')).toBeInTheDocument();
    });
  });

  describe('batch details drawer / navigation', () => {
    it('navigates to the linked session for a session-type batch instead of opening the drawer', async () => {
      const sessionBatch = { ...makeBatch(1), type: 'session' as const, linkedSessionId: 'sess1' };
      (axios.get as any).mockResolvedValue({ data: [sessionBatch] });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());
      fireEvent.click(screen.getByTitle('Open session'));

      expect(mockNavigate).toHaveBeenCalledWith('/sessions/sess1');
    });

    it('does nothing for a session-type batch with no linkedSessionId', async () => {
      const sessionBatch = { ...makeBatch(1), type: 'session' as const, linkedSessionId: undefined };
      (axios.get as any).mockResolvedValue({ data: [sessionBatch] });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());
      fireEvent.click(screen.getByTitle('Open session'));

      expect(mockNavigate).not.toHaveBeenCalled();
    });

    it('opens the drawer for a manual batch and shows its student roster', async () => {
      const batch = makeBatch(1);
      const detailed = {
        ...batch,
        students: [
          { rollNumber: 'R1', name: 'Alice' },
          { rollNumber: 'R2', name: 'Bob' },
        ],
      };
      (axios.get as any).mockImplementation((url: string) => {
        if (url === '/api/admin/batches/b1') return Promise.resolve({ data: detailed });
        return Promise.resolve({ data: [batch] });
      });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());
      fireEvent.click(screen.getByTitle('View Roster'));

      await waitFor(() => expect(axios.get).toHaveBeenCalledWith('/api/admin/batches/b1'));
      expect(await screen.findByText('Alice')).toBeInTheDocument();
      expect(screen.getByText('Bob')).toBeInTheDocument();
      // Total students stat should reflect the roster length.
      expect(screen.getByText('2')).toBeInTheDocument();
    });

    it('closes the drawer via its close button', async () => {
      const batch = makeBatch(1);
      const detailed = { ...batch, students: [{ rollNumber: 'R1', name: 'Alice' }] };
      (axios.get as any).mockImplementation((url: string) => {
        if (url === '/api/admin/batches/b1') return Promise.resolve({ data: detailed });
        return Promise.resolve({ data: [batch] });
      });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());
      fireEvent.click(screen.getByTitle('View Roster'));
      const alice = await screen.findByText('Alice');
      expect(alice).toBeInTheDocument();

      const closeBtn = alice.closest('.drawer-panel')!.querySelector('button')!;
      fireEvent.click(closeBtn);
      await waitFor(() => expect(screen.queryByText('Alice')).not.toBeInTheDocument());
    });

    it('shows an error toast and closes the drawer when batch detail fetch fails', async () => {
      const batch = makeBatch(1);
      (axios.get as any).mockImplementation((url: string) => {
        if (url === '/api/admin/batches/b1') return Promise.reject(new Error('fail'));
        return Promise.resolve({ data: [batch] });
      });

      renderComponent();
      await waitFor(() => expect(screen.getByText('Batch 1')).toBeInTheDocument());
      fireEvent.click(screen.getByTitle('View Roster'));

      await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Failed to fetch batch details'));
      await waitFor(() => expect(screen.queryByText('Loading...')).not.toBeInTheDocument());
    });
  });
});
