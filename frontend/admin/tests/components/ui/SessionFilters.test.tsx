import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SessionFilters from '../../../src/components/ui/SessionFilters';

const LOCATIONS = [
  { _id: 'loc1', name: 'Room 101' },
  { _id: 'loc2', name: 'Room 202' },
];

describe('SessionFilters', () => {
  beforeEach(() => {
    sessionStorage.clear();
  });

  it('renders the Filters button with no active-count badge when nothing is applied', () => {
    render(<SessionFilters locations={LOCATIONS} />);
    expect(screen.getByText('Filters')).toBeInTheDocument();
    // No badge span with a count next to it.
    expect(screen.queryByText('1')).not.toBeInTheDocument();
    expect(screen.queryByText('2')).not.toBeInTheDocument();
  });

  it('opens the popup on click and shows all locations plus "All Locations"', () => {
    render(<SessionFilters locations={LOCATIONS} />);
    fireEvent.click(screen.getByText('Filters'));

    expect(screen.getByText('Filter Sessions')).toBeInTheDocument();
    expect(screen.getByText('All Locations')).toBeInTheDocument();
    expect(screen.getByText('Room 101')).toBeInTheDocument();
    expect(screen.getByText('Room 202')).toBeInTheDocument();
  });

  it('closes the popup via the X button without applying anything', () => {
    render(<SessionFilters locations={LOCATIONS} />);
    fireEvent.click(screen.getByText('Filters'));
    expect(screen.getByText('Filter Sessions')).toBeInTheDocument();

    // The header close button has no accessible name — it's the only
    // button inside the header block, find it via the SVG icon parent.
    const closeButtons = document.querySelectorAll('button');
    const closeBtn = Array.from(closeButtons).find((b) => b.querySelector('svg') && b.className.includes('w-8'));
    fireEvent.click(closeBtn!);

    expect(screen.queryByText('Filter Sessions')).not.toBeInTheDocument();
  });

  it('selects a location and date, applies, and calls onFilterChange + persists to sessionStorage', () => {
    const onFilterChange = vi.fn();
    render(<SessionFilters locations={LOCATIONS} onFilterChange={onFilterChange} />);
    fireEvent.click(screen.getByText('Filters'));

    fireEvent.click(screen.getByText('Room 202'));
    const dateInput = document.querySelector('input[type="date"]')!;
    fireEvent.change(dateInput, { target: { value: '2026-08-20' } });

    fireEvent.click(screen.getByText('Apply Filters'));

    expect(onFilterChange).toHaveBeenCalledWith({ locationId: 'loc2', date: '2026-08-20' });
    expect(JSON.parse(sessionStorage.getItem('sessionFilters')!)).toEqual({ locationId: 'loc2', date: '2026-08-20' });
    // Popup closes after apply.
    expect(screen.queryByText('Filter Sessions')).not.toBeInTheDocument();
  });

  it('shows an active-filter count badge once a filter has been applied', () => {
    render(<SessionFilters locations={LOCATIONS} />);
    fireEvent.click(screen.getByText('Filters'));
    fireEvent.click(screen.getByText('Room 101'));
    fireEvent.click(screen.getByText('Apply Filters'));

    expect(screen.getByText('1')).toBeInTheDocument();
  });

  it('the Clear Date button appears once a date is picked and clears it', () => {
    render(<SessionFilters locations={LOCATIONS} />);
    fireEvent.click(screen.getByText('Filters'));

    expect(screen.queryByText('Clear Date')).not.toBeInTheDocument();
    const dateInput = document.querySelector('input[type="date"]')!;
    fireEvent.change(dateInput, { target: { value: '2026-08-20' } });
    expect(screen.getByText('Clear Date')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Clear Date'));
    expect(screen.queryByText('Clear Date')).not.toBeInTheDocument();
  });

  it('Reset All clears both fields back to defaults within the popup', () => {
    render(<SessionFilters locations={LOCATIONS} />);
    fireEvent.click(screen.getByText('Filters'));

    fireEvent.click(screen.getByText('Room 101'));
    const dateInput = document.querySelector('input[type="date"]')! as HTMLInputElement;
    fireEvent.change(dateInput, { target: { value: '2026-08-20' } });
    expect(screen.getByText('Clear Date')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Reset All'));
    expect(screen.queryByText('Clear Date')).not.toBeInTheDocument();
  });

  it('Cancel closes the popup without applying or persisting changes', () => {
    const onFilterChange = vi.fn();
    render(<SessionFilters locations={LOCATIONS} onFilterChange={onFilterChange} />);
    fireEvent.click(screen.getByText('Filters'));
    fireEvent.click(screen.getByText('Room 101'));

    fireEvent.click(screen.getByText('Cancel'));

    expect(onFilterChange).not.toHaveBeenCalled();
    expect(sessionStorage.getItem('sessionFilters')).toBeNull();
    expect(screen.queryByText('Filter Sessions')).not.toBeInTheDocument();
  });

  it('closes the popup when the Escape key is pressed', () => {
    render(<SessionFilters locations={LOCATIONS} />);
    fireEvent.click(screen.getByText('Filters'));
    expect(screen.getByText('Filter Sessions')).toBeInTheDocument();

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByText('Filter Sessions')).not.toBeInTheDocument();
  });

  it('closes the popup when the backdrop is clicked, but not when the panel itself is clicked', () => {
    render(<SessionFilters locations={LOCATIONS} />);
    fireEvent.click(screen.getByText('Filters'));

    // Click inside the panel — should NOT close (event.stopPropagation).
    fireEvent.click(screen.getByText('Filter Sessions'));
    expect(screen.getByText('Filter Sessions')).toBeInTheDocument();

    // Click the backdrop itself (the outer fixed overlay).
    const backdrop = document.querySelector('.fixed.inset-0')!;
    fireEvent.click(backdrop);
    expect(screen.queryByText('Filter Sessions')).not.toBeInTheDocument();
  });

  it('restores a previously saved filter from sessionStorage on mount', () => {
    sessionStorage.setItem('sessionFilters', JSON.stringify({ locationId: 'loc1', date: '2026-01-01' }));
    render(<SessionFilters locations={LOCATIONS} />);
    // Active count badge reflects the restored filters immediately.
    expect(screen.getByText('2')).toBeInTheDocument();
  });

  it('falls back to empty filters when sessionStorage holds invalid JSON', () => {
    sessionStorage.setItem('sessionFilters', '{not-json');
    render(<SessionFilters locations={LOCATIONS} />);
    expect(screen.queryByText('1')).not.toBeInTheDocument();
    expect(screen.queryByText('2')).not.toBeInTheDocument();
  });
});
