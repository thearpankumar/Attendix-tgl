import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import axios from 'axios';
import { toast } from 'react-toastify';
import BatchRosterEditorModal from '../../../src/components/batches/BatchRosterEditorModal';
import { describeDeleteImpact } from '../../../src/utils/batchDelete';

const BATCH = { _id: 'b1', name: 'CS 2026' };
const ROSTER = {
  students: [
    { id: 's1', name: 'Alice', rollNumber: 'R001', collegeName: 'X', email: 'a@x.test' },
    { id: 's2', name: 'Bob', rollNumber: 'R002', collegeName: null, email: null },
  ],
};

const renderModal = (props: Partial<React.ComponentProps<typeof BatchRosterEditorModal>> = {}) => {
  const onClose = vi.fn();
  const onChanged = vi.fn();
  render(<BatchRosterEditorModal batch={BATCH} onClose={onClose} onChanged={onChanged} {...props} />);
  return { onClose, onChanged };
};

describe('BatchRosterEditorModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (axios.get as any).mockResolvedValue({ data: ROSTER });
  });

  it('loads the roster into editable rows', async () => {
    renderModal();
    await waitFor(() => expect(screen.getAllByTestId('roster-row')).toHaveLength(2));
    expect(axios.get).toHaveBeenCalledWith('/api/admin/batches/b1');
    expect((screen.getByLabelText('Name for row 1') as HTMLInputElement).value).toBe('Alice');
    expect((screen.getByRole('button', { name: 'Save changes' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('edits a row, adds a row and removes a row, then saves via the row endpoints', async () => {
    (axios.post as any).mockResolvedValue({ data: { id: 's3' } });
    const { onChanged, onClose } = renderModal();
    await waitFor(() => expect(screen.getAllByTestId('roster-row')).toHaveLength(2));

    fireEvent.change(screen.getByLabelText('Name for row 1'), { target: { value: 'Alice B' } });
    fireEvent.click(screen.getByRole('button', { name: 'Remove row 2' }));
    fireEvent.click(screen.getByRole('button', { name: /Add row/ }));
    fireEvent.change(screen.getByLabelText('Roll number for row 3'), { target: { value: 'R003' } });
    fireEvent.change(screen.getByLabelText('Name for row 3'), { target: { value: 'Cara' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));
    await waitFor(() => expect(axios.delete).toHaveBeenCalledWith('/api/admin/batches/b1/students/s2'));
    expect(axios.patch).toHaveBeenCalledWith('/api/admin/batches/b1/students/s1', expect.objectContaining({ name: 'Alice B', rollNumber: 'R001' }));
    expect(axios.post).toHaveBeenCalledWith('/api/admin/batches/b1/students', expect.objectContaining({ name: 'Cara', rollNumber: 'R003', email: null }));
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('Roster saved (3 changes)'));

    fireEvent.click(screen.getByText('Close', { selector: 'button.btn' }));
    expect(onChanged).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it('flags a duplicate roll number (case-insensitive) and blocks saving', async () => {
    renderModal();
    await waitFor(() => expect(screen.getAllByTestId('roster-row')).toHaveLength(2));
    fireEvent.change(screen.getByLabelText('Roll number for row 2'), { target: { value: 'r001' } });
    expect(screen.getAllByText('Duplicate roll number in this batch').length).toBeGreaterThan(0);
    expect((screen.getByRole('button', { name: 'Save changes' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('shows the server error on the failing row', async () => {
    (axios.patch as any).mockRejectedValue({ response: { data: { message: 'A student with roll number already exists' } } });
    renderModal();
    await waitFor(() => expect(screen.getAllByTestId('roster-row')).toHaveLength(2));
    fireEvent.change(screen.getByLabelText('Name for row 1'), { target: { value: 'Alice B' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));
    await waitFor(() => expect(screen.getByText('A student with roll number already exists')).toBeInTheDocument());
    expect(toast.error).toHaveBeenCalled();
  });

  it('upload flow: parse -> preview diff -> confirm applies with the chosen mode', async () => {
    const parsed = { students: [{ name: 'Alice', rollNumber: 'R001' }, { name: 'Cara', rollNumber: 'R003' }], errors: ['Duplicate roll number ignored'] };
    const diff = {
      mode: 'replace',
      added: [{ name: 'Cara', rollNumber: 'R003' }],
      updated: [],
      removed: [{ id: 's2', name: 'Bob', rollNumber: 'R002' }],
      unchanged: 1,
    };
    (axios.post as any).mockImplementation((url: string) =>
      Promise.resolve({ data: url.endsWith('/parse') ? parsed : diff })
    );
    renderModal();
    await waitFor(() => expect(screen.getAllByTestId('roster-row')).toHaveLength(2));

    fireEvent.click(screen.getByRole('tab', { name: /Update from file/ }));
    fireEvent.click(screen.getByLabelText(/Replace roster/));
    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [new File(['x'], 'roster.csv', { type: 'text/csv' })] } });
    fireEvent.click(screen.getByRole('button', { name: 'Preview changes' }));

    await waitFor(() => expect(screen.getByTestId('roster-diff')).toBeInTheDocument());
    expect(screen.getByText('1 to add')).toBeInTheDocument();
    expect(screen.getByText('1 to remove')).toBeInTheDocument();
    expect(screen.getByText('1 unchanged')).toBeInTheDocument();
    expect(screen.getByText('R002 — Bob')).toBeInTheDocument();
    expect(screen.getByText(/past attendance records and exports are kept/)).toBeInTheDocument();
    expect(axios.post).toHaveBeenCalledWith('/api/admin/batches/b1/students/preview', { mode: 'replace', students: parsed.students });

    fireEvent.click(screen.getByRole('button', { name: 'Replace roster' }));
    await waitFor(() => expect(axios.put).toHaveBeenCalledWith('/api/admin/batches/b1/students', { mode: 'replace', students: parsed.students, source: 'excel' }));
    expect(toast.success).toHaveBeenCalledWith('Roster updated');
  });

  it('rejects an unsupported file type', async () => {
    renderModal();
    await waitFor(() => expect(screen.getAllByTestId('roster-row')).toHaveLength(2));
    fireEvent.click(screen.getByRole('tab', { name: /Update from file/ }));
    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [new File(['x'], 'notes.txt')] } });
    expect(toast.error).toHaveBeenCalledWith('Please upload a valid .csv, .xlsx, .xls, or .ods file');
  });
});

describe('describeDeleteImpact', () => {
  it('describes archive vs permanent delete and the generic fallback', () => {
    expect(describeDeleteImpact('B', 4, { sessionCount: 2, ruleCount: 0, activeRuleCount: 0, mode: 'archived' })).toMatch(/used by 2 sessions\./);
    expect(describeDeleteImpact('B', 1, { sessionCount: 0, ruleCount: 0, activeRuleCount: 0, mode: 'deleted' })).toMatch(/its 1 student will be permanently deleted/);
    expect(describeDeleteImpact('B', 1, null)).toMatch(/archived instead/);
  });
});
