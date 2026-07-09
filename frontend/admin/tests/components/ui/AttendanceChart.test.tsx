import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import AttendanceChart from '../../../src/components/ui/AttendanceChart';
import axios from 'axios';

describe('AttendanceChart Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders demo data initially', async () => {
    (axios.get as any).mockResolvedValue({ data: [] });
    render(<AttendanceChart />);
    
    // Wait for initial useEffect fetch to complete
    await waitFor(() => expect(axios.get).toHaveBeenCalled());
    
    // Switch to Overview and Daily (default)
    expect(screen.getByText('Overview')).toBeInTheDocument();
    
    // Since demo data is auto-generated, we should see it
    expect(screen.getByText('Attendance Overview')).toBeInTheDocument();
    expect(screen.getByText('demo data')).toBeInTheDocument();
    
    // Switch to weekly
    fireEvent.click(screen.getByText('Weekly'));
    // Switch to monthly
    fireEvent.click(screen.getByText('Monthly'));
    
    // Switch to date mode
    fireEvent.click(screen.getByText('By date'));
    expect(screen.getByText('Pick date')).toBeInTheDocument();
  });

  it('switches to live data and handles empty states', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('attendance-series')) return Promise.resolve({ data: [] });
      if (url.includes('locations')) return Promise.resolve({ data: [] });
      return Promise.resolve({ data: [] });
    });

    render(<AttendanceChart />);
    await waitFor(() => expect(axios.get).toHaveBeenCalled());

    // Switch to live
    fireEvent.click(screen.getByText('Live'));
    await waitFor(() => {
      expect(screen.getByText('No attendance recorded yet.')).toBeInTheDocument();
    });
  });

  it('can open modal and apply date filters', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('sessions-by-date')) {
        return Promise.resolve({ data: [{ date: '2023-01-01', location: 'ECE Hall', session: 'Test', count: 10, sessionId: 's1' }] });
      }
      return Promise.resolve({ data: [] });
    });

    render(<AttendanceChart />);
    await waitFor(() => expect(axios.get).toHaveBeenCalled());

    // Switch to live
    fireEvent.click(screen.getByText('Live'));
    
    // Switch to date mode
    fireEvent.click(screen.getByText('By date'));
    
    // Open modal
    fireEvent.click(screen.getByText('Pick date'));
    
    await waitFor(() => expect(screen.getByText('Select date & sessions')).toBeInTheDocument());
    
    // Change date
    const dateInput = document.querySelector('input[type="date"]');
    if (dateInput) fireEvent.change(dateInput, { target: { value: '2023-01-01' } });
    
    await waitFor(() => {
      const checkbox = document.querySelector('input[type="checkbox"]');
      if (checkbox) fireEvent.click(checkbox);
    });

    fireEvent.click(screen.getByText('Check now'));
    
    await waitFor(() => {
      expect(screen.queryByText('Select date & sessions')).not.toBeInTheDocument();
    });
    
    // Switch to location grouping
    fireEvent.click(screen.getByText('By location'));
  });
});
