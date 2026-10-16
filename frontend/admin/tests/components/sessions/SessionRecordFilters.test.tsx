import { render, screen, fireEvent, within } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SessionRecordFilters, { emptyRecordFilters } from '../../../src/components/sessions/SessionRecordFilters';
import type { SessionRecordFiltersState } from '../../../src/components/sessions/SessionRecordFilters';

// batchA (manual) -> Room 1, DSA, 2-4 PM
// batchB (session) -> Room 2, DSA, 4-6 PM
// batchC (session) -> Room 1, ML,  2-4 PM
const SESSIONS = [
  { batchId: 'batchA', locationName: 'Room 1', track: 'DSA', sessionTimeRaw: '2-4 PM' },
  { excelBatchId: 'batchB', locationName: 'Room 2', track: 'DSA', sessionTimeRaw: '4-6 PM' },
  { excelBatchId: 'batchC', locationName: 'Room 1', track: 'ML', sessionTimeRaw: '2-4 PM' },
];

const BATCHES = [
  { _id: 'batchA', name: 'Batch A', type: 'manual' as const },
  { _id: 'batchB', name: 'Batch B', type: 'session' as const },
  { _id: 'batchC', name: 'Batch C', type: 'session' as const },
];

const noop = () => {};

const renderFilters = (filters: SessionRecordFiltersState) => {
  const onChange = vi.fn();
  const utils = render(
    <SessionRecordFilters
      allSessions={SESSIONS}
      batches={BATCHES}
      filters={filters}
      onChange={onChange}
      onExportFiltered={noop}
      exporting={false}
      exportCount={0}
    />
  );
  return { onChange, ...utils };
};

describe('SessionRecordFilters', () => {
  it('renders Classes as a multi-select (not a native dropdown)', () => {
    renderFilters(emptyRecordFilters);
    const classesInput = screen.getByLabelText(/Select Classes/i);
    expect(classesInput.tagName).toBe('INPUT');
  });

  it('with no filters active, every dropdown offers every option', () => {
    renderFilters(emptyRecordFilters);

    fireEvent.focus(screen.getByLabelText(/Select Classes/i));
    expect(screen.getByText('Room 1')).toBeInTheDocument();
    expect(screen.getByText('Room 2')).toBeInTheDocument();

    fireEvent.focus(screen.getByLabelText(/Select Tracks/i));
    expect(screen.getByText('DSA')).toBeInTheDocument();
    expect(screen.getByText('ML')).toBeInTheDocument();

    fireEvent.focus(screen.getByLabelText(/Select Batches/i));
    expect(screen.getByText('Batch A')).toBeInTheDocument();
    expect(screen.getByText(/Batch B \(Session Batch\)/)).toBeInTheDocument();
    expect(screen.getByText(/Batch C \(Session Batch\)/)).toBeInTheDocument();
  });

  it('selecting a Batch narrows the Classes/Tracks/Time Slots options to just that batch', () => {
    renderFilters({ ...emptyRecordFilters, batchIds: ['batchA'] });

    fireEvent.focus(screen.getByLabelText(/Select Classes/i));
    expect(screen.getByText('Room 1')).toBeInTheDocument();
    expect(screen.queryByText('Room 2')).not.toBeInTheDocument();

    fireEvent.focus(screen.getByLabelText(/Select Tracks/i));
    expect(screen.getByText('DSA')).toBeInTheDocument();
    expect(screen.queryByText('ML')).not.toBeInTheDocument();

    fireEvent.focus(screen.getByLabelText(/Select Time Slots/i));
    expect(screen.getByText('2-4 PM')).toBeInTheDocument();
    expect(screen.queryByText('4-6 PM')).not.toBeInTheDocument();
  });

  it('selecting a Class narrows the Batches/Tracks/Time Slots options — the reverse direction', () => {
    renderFilters({ ...emptyRecordFilters, classLabels: ['Room 2'] });

    fireEvent.focus(screen.getByLabelText(/Select Batches/i));
    expect(screen.getByText(/Batch B \(Session Batch\)/)).toBeInTheDocument();
    expect(screen.queryByText('Batch A')).not.toBeInTheDocument();
    expect(screen.queryByText(/Batch C \(Session Batch\)/)).not.toBeInTheDocument();

    fireEvent.focus(screen.getByLabelText(/Select Tracks/i));
    expect(screen.getByText('DSA')).toBeInTheDocument();
    expect(screen.queryByText('ML')).not.toBeInTheDocument();

    fireEvent.focus(screen.getByLabelText(/Select Time Slots/i));
    expect(screen.getByText('4-6 PM')).toBeInTheDocument();
    expect(screen.queryByText('2-4 PM')).not.toBeInTheDocument();
  });

  it('selecting a Track independently narrows Classes/Time Slots/Batches too', () => {
    renderFilters({ ...emptyRecordFilters, tracks: ['ML'] });

    // Only batchC (Room 1, ML, 2-4 PM) matches track=ML.
    fireEvent.focus(screen.getByLabelText(/Select Classes/i));
    expect(screen.getByText('Room 1')).toBeInTheDocument();
    expect(screen.queryByText('Room 2')).not.toBeInTheDocument();

    fireEvent.focus(screen.getByLabelText(/Select Batches/i));
    expect(screen.getByText(/Batch C \(Session Batch\)/)).toBeInTheDocument();
    expect(screen.queryByText('Batch A')).not.toBeInTheDocument();
    expect(screen.queryByText(/Batch B \(Session Batch\)/)).not.toBeInTheDocument();
  });

  it('a facet never filters against its own current selection', () => {
    // DSA already selected — the Tracks dropdown must still offer both DSA
    // (shown as selected/checked) and ML, not just what's already picked.
    renderFilters({ ...emptyRecordFilters, tracks: ['DSA'] });
    fireEvent.focus(screen.getByLabelText(/Select Tracks/i));
    const dropdown = document.querySelector('.multiselect-dropdown')!;
    expect(within(dropdown as HTMLElement).getByText('DSA')).toBeInTheDocument();
    expect(within(dropdown as HTMLElement).getByText('ML')).toBeInTheDocument();
  });

  it('"Deselect All" clears just that one facet', () => {
    const { onChange } = renderFilters({ ...emptyRecordFilters, classLabels: ['Room 1', 'Room 2'], tracks: ['DSA'] });
    const classesSection = screen.getByLabelText(/Select Classes/i).closest<HTMLElement>('.form-group')!;
    fireEvent.click(within(classesSection).getByText('Deselect All'));
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ classLabels: [], tracks: ['DSA'] }));
  });

  it('toggling a class option calls onChange with the updated classLabels array', () => {
    const { onChange } = renderFilters(emptyRecordFilters);
    fireEvent.focus(screen.getByLabelText(/Select Classes/i));
    fireEvent.click(screen.getByText('Room 1'));
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ classLabels: ['Room 1'] }));
  });

  describe('collapsible card', () => {
    beforeEach(() => localStorage.clear());

    const toggle = () => screen.getByRole('button', { name: /^Filters/ });

    it('is expanded by default with aria-expanded and aria-controls wired to the body', () => {
      renderFilters(emptyRecordFilters);
      const btn = toggle();
      expect(btn).toHaveAttribute('aria-expanded', 'true');
      const body = document.getElementById(btn.getAttribute('aria-controls')!);
      expect(body).not.toBeNull();
      expect(body).not.toHaveClass('is-collapsed');
      expect(screen.queryByTestId('filters-summary')).not.toBeInTheDocument();
    });

    it('collapses and expands on click, keeping the body mounted and Export visible', () => {
      renderFilters(emptyRecordFilters);
      fireEvent.click(toggle());
      expect(toggle()).toHaveAttribute('aria-expanded', 'false');
      const body = document.getElementById(toggle().getAttribute('aria-controls')!)!;
      expect(body).toHaveClass('is-collapsed');
      expect(screen.getByLabelText(/Select Batches/i)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /Export Filtered/ })).toBeVisible();
      fireEvent.click(toggle());
      expect(toggle()).toHaveAttribute('aria-expanded', 'true');
      expect(body).not.toHaveClass('is-collapsed');
    });

    it('is a real button, so Enter/Space activation comes from the platform', () => {
      renderFilters(emptyRecordFilters);
      expect(toggle().tagName).toBe('BUTTON');
      expect(toggle()).toHaveAttribute('type', 'button');
    });

    it('does not toggle when Export is clicked', () => {
      const onExport = vi.fn();
      render(
        <SessionRecordFilters allSessions={SESSIONS} batches={BATCHES} filters={emptyRecordFilters}
          onChange={vi.fn()} onExportFiltered={onExport} exporting={false} exportCount={3} />
      );
      fireEvent.click(screen.getByRole('button', { name: /Export Filtered \(3\)/ }));
      expect(onExport).toHaveBeenCalledTimes(1);
      expect(toggle()).toHaveAttribute('aria-expanded', 'true');
    });

    it('shows "No filters" when collapsed with nothing selected', () => {
      renderFilters(emptyRecordFilters);
      fireEvent.click(toggle());
      expect(screen.getByTestId('filters-summary')).toHaveTextContent('No filters');
    });

    it('summarises the number of active filters when collapsed', () => {
      renderFilters({ ...emptyRecordFilters, batchIds: ['batchA', 'batchB'], tracks: ['DSA'], search: ' ann ' });
      fireEvent.click(toggle());
      expect(screen.getByTestId('filters-summary')).toHaveTextContent('3 filters active');
    });

    it('uses the singular for one active filter and ignores a blank search', () => {
      renderFilters({ ...emptyRecordFilters, markingStatus: 'partial', search: '   ' });
      fireEvent.click(toggle());
      expect(screen.getByTestId('filters-summary')).toHaveTextContent('1 filter active');
    });

    it('persists the collapsed state across remounts and defaults to expanded when storage throws', () => {
      const first = renderFilters(emptyRecordFilters);
      fireEvent.click(toggle());
      first.unmount();
      renderFilters(emptyRecordFilters);
      expect(toggle()).toHaveAttribute('aria-expanded', 'false');
    });

    it('still works when localStorage is unavailable', () => {
      const getSpy = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('blocked'); });
      const setSpy = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('blocked'); });
      renderFilters(emptyRecordFilters);
      expect(toggle()).toHaveAttribute('aria-expanded', 'true');
      fireEvent.click(toggle());
      expect(toggle()).toHaveAttribute('aria-expanded', 'false');
      getSpy.mockRestore();
      setSpy.mockRestore();
    });

    it('does not change filter behaviour while collapsed', () => {
      const { onChange } = renderFilters({ ...emptyRecordFilters, tracks: ['DSA'] });
      fireEvent.click(toggle());
      expect(onChange).not.toHaveBeenCalled();
    });
  });
});
