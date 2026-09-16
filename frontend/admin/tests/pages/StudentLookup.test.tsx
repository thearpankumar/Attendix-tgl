import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import StudentLookup from '../../src/pages/StudentLookup';
import axios from 'axios';
import { MemoryRouter } from 'react-router';

const mockNavigate = vi.fn();
vi.mock('react-router', async () => {
  const actual = await vi.importActual('react-router');
  return { ...actual, useNavigate: () => mockNavigate };
});

const renderComponent = () =>
  render(
    <MemoryRouter>
      <StudentLookup />
    </MemoryRouter>
  );

const emptyStudentsPage = { students: [], hasMore: false };

describe('StudentLookup', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads the plain student list on mount', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url === '/api/admin/students') {
        return Promise.resolve({
          data: {
            students: [
              { studentId: 's1', name: 'Alice', rollNumber: 'R1', email: null, batchId: 'b1', batchName: 'Batch A', present: 3, totalSessions: 4, percentage: 75 },
            ],
            hasMore: false,
          },
        });
      }
      return Promise.resolve({ data: emptyStudentsPage });
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Alice')).toBeInTheDocument());
    expect(axios.get).toHaveBeenCalledWith(
      '/api/admin/students',
      expect.objectContaining({ params: expect.objectContaining({ search: undefined }) })
    );
  });

  it('a single roll number with no separator stays in plain fuzzy search — no bulk-lookup call', async () => {
    (axios.get as any).mockResolvedValue({ data: emptyStudentsPage });
    renderComponent();
    await waitFor(() => expect(axios.get).toHaveBeenCalled());

    fireEvent.change(screen.getByPlaceholderText(/paste roll numbers/i), { target: { value: '21B91A0501' } });

    await waitFor(
      () =>
        expect(axios.get).toHaveBeenCalledWith(
          '/api/admin/students',
          expect.objectContaining({ params: expect.objectContaining({ search: '21B91A0501' }) })
        ),
      { timeout: 2000 }
    );
    expect(axios.post).not.toHaveBeenCalledWith('/api/admin/students/lookup', expect.anything());
  });

  it('comma-separated roll numbers in the search box switch to bulk lookup and show results', async () => {
    (axios.get as any).mockResolvedValue({ data: emptyStudentsPage });
    (axios.post as any).mockImplementation((url: string) => {
      if (url === '/api/admin/students/lookup') {
        return Promise.resolve({
          data: {
            matches: [
              {
                rollNumberQueried: 'R1',
                studentId: 's1',
                batchId: 'b1',
                batchName: 'Batch A',
                name: 'Alice',
                email: null,
                present: 3,
                totalSessions: 4,
                percentage: 75,
              },
            ],
            aggregates: [{ rollNumber: 'R1', totalPresent: 3, totalSessions: 4, percentage: 75, matchedBatchCount: 1 }],
            notFound: ['R2'],
          },
        });
      }
      return Promise.resolve({ data: {} });
    });

    renderComponent();
    await waitFor(() => expect(axios.get).toHaveBeenCalled());

    fireEvent.change(screen.getByPlaceholderText(/paste roll numbers/i), { target: { value: 'R1, R2' } });

    await waitFor(
      () =>
        expect(axios.post).toHaveBeenCalledWith(
          '/api/admin/students/lookup',
          expect.objectContaining({ rollNumbers: ['R1', 'R2'] })
        ),
      { timeout: 2000 }
    );

    await waitFor(() => expect(screen.getByText('R1')).toBeInTheDocument());
    expect(screen.getByText(/Not Found \(1\)/)).toBeInTheDocument();
    expect(screen.getByText('R2')).toBeInTheDocument();
    // The plain student-list table is hidden while in bulk-lookup mode.
    expect(screen.queryByText(/No Students Found/i)).not.toBeInTheDocument();
  });

  it('uploading a file extracts roll numbers, fills the search box, and shows results without a second lookup call', async () => {
    (axios.get as any).mockResolvedValue({ data: emptyStudentsPage });
    (axios.post as any).mockImplementation((url: string) => {
      if (url === '/api/admin/students/lookup/file') {
        return Promise.resolve({
          data: {
            matches: [
              {
                rollNumberQueried: 'R1',
                studentId: 's1',
                batchId: 'b1',
                batchName: 'Batch A',
                name: 'Alice',
                email: null,
                present: 3,
                totalSessions: 4,
                percentage: 75,
              },
              {
                rollNumberQueried: 'R2',
                studentId: 's2',
                batchId: 'b1',
                batchName: 'Batch A',
                name: 'Bob',
                email: null,
                present: 2,
                totalSessions: 4,
                percentage: 50,
              },
            ],
            aggregates: [
              { rollNumber: 'R1', totalPresent: 3, totalSessions: 4, percentage: 75, matchedBatchCount: 1 },
              { rollNumber: 'R2', totalPresent: 2, totalSessions: 4, percentage: 50, matchedBatchCount: 1 },
            ],
            notFound: [],
          },
        });
      }
      return Promise.resolve({ data: {} });
    });

    renderComponent();
    await waitFor(() => expect(axios.get).toHaveBeenCalled());

    const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement;
    const rosterFile = new File(['Roll Number\nR1\nR2'], 'roster.csv', { type: 'text/csv' });
    fireEvent.change(fileInput, { target: { files: [rosterFile] } });

    await waitFor(() =>
      expect(axios.post).toHaveBeenCalledWith(
        '/api/admin/students/lookup/file',
        expect.any(FormData),
        expect.objectContaining({ headers: { 'Content-Type': 'multipart/form-data' } })
      )
    );

    // Search box now shows the roll numbers the upload resolved, comma-separated.
    const searchBox = screen.getByPlaceholderText(/paste roll numbers/i) as HTMLInputElement;
    await waitFor(() => expect(searchBox.value).toBe('R1, R2'));

    // Results render from the upload response directly — no extra POST beyond the one file-upload call.
    await waitFor(() => expect(screen.getByText('R1')).toBeInTheDocument());
    expect(screen.getByText('R2')).toBeInTheDocument();
    expect(axios.post).toHaveBeenCalledTimes(1);
  });

  it('clearing the search box returns to the plain student list', async () => {
    (axios.get as any).mockResolvedValue({ data: emptyStudentsPage });
    (axios.post as any).mockResolvedValue({ data: { matches: [], aggregates: [], notFound: ['R2'] } });

    renderComponent();
    await waitFor(() => expect(axios.get).toHaveBeenCalled());

    const searchBox = screen.getByPlaceholderText(/paste roll numbers/i);
    fireEvent.change(searchBox, { target: { value: 'R1, R2' } });
    await waitFor(() => expect(screen.getByText(/Not Found \(1\)/)).toBeInTheDocument());

    fireEvent.click(screen.getByTitle('Clear search'));
    await waitFor(() => expect(screen.queryByText(/Not Found/)).not.toBeInTheDocument());
    expect(screen.getByText(/No Students Found/i)).toBeInTheDocument();
  });
});
