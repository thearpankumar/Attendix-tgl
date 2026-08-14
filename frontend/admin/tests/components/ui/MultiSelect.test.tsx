import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import MultiSelect from '../../../src/components/ui/MultiSelect';

const OPTIONS = [
  { value: 'a', label: 'Jane Mentor' },
  { value: 'b', label: 'John Smith' },
  { value: 'c', label: 'Priya Sharma' },
];

describe('MultiSelect', () => {
  it('shows selected options as removable chips', () => {
    render(<MultiSelect options={OPTIONS} selected={['a', 'b']} onChange={vi.fn()} />);
    expect(screen.getByText('Jane Mentor')).toBeInTheDocument();
    expect(screen.getByText('John Smith')).toBeInTheDocument();
    expect(screen.queryByText('Priya Sharma')).not.toBeInTheDocument();
  });

  it('opens the dropdown and adds an option on click', () => {
    const onChange = vi.fn();
    render(<MultiSelect id="mentors" options={OPTIONS} selected={[]} onChange={onChange} placeholder="Search…" />);

    fireEvent.focus(screen.getByPlaceholderText('Search…'));
    fireEvent.click(screen.getByText('Priya Sharma'));

    expect(onChange).toHaveBeenCalledWith(['c']);
  });

  it('removes an option when its chip is clicked', () => {
    const onChange = vi.fn();
    render(<MultiSelect options={OPTIONS} selected={['a', 'c']} onChange={onChange} />);

    fireEvent.click(screen.getByLabelText('Remove Jane Mentor'));

    expect(onChange).toHaveBeenCalledWith(['c']);
  });

  it('filters the dropdown by search text', () => {
    render(<MultiSelect options={OPTIONS} selected={[]} onChange={vi.fn()} placeholder="Search…" />);
    const input = screen.getByPlaceholderText('Search…');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'john' } });

    expect(screen.getByText('John Smith')).toBeInTheDocument();
    expect(screen.queryByText('Jane Mentor')).not.toBeInTheDocument();
    expect(screen.queryByText('Priya Sharma')).not.toBeInTheDocument();
  });

  it('shows the empty message when nothing matches', () => {
    render(<MultiSelect options={OPTIONS} selected={[]} onChange={vi.fn()} placeholder="Search…" emptyMessage="No mentors match your search" />);
    const input = screen.getByPlaceholderText('Search…');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'zzz-nomatch' } });

    expect(screen.getByText('No mentors match your search')).toBeInTheDocument();
  });

  it('removes the last chip on Backspace when the search box is empty', () => {
    const onChange = vi.fn();
    render(<MultiSelect options={OPTIONS} selected={['a', 'b']} onChange={onChange} />);

    fireEvent.focus(screen.getByRole('textbox'));
    fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Backspace' });

    expect(onChange).toHaveBeenCalledWith(['a']);
  });

  it('closes the dropdown when clicking outside', () => {
    render(
      <div>
        <MultiSelect options={OPTIONS} selected={[]} onChange={vi.fn()} placeholder="Search…" />
        <button>outside</button>
      </div>
    );
    fireEvent.focus(screen.getByPlaceholderText('Search…'));
    expect(screen.getByText('John Smith')).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByText('outside'));
    expect(screen.queryByText('John Smith')).not.toBeInTheDocument();
  });
});
