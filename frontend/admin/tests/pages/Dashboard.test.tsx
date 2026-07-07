import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Dashboard from '../../src/pages/Dashboard';
import axios from 'axios';
import { MemoryRouter } from 'react-router-dom';
import { toast } from 'react-toastify';

describe('Dashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const renderComponent = () => render(
    <MemoryRouter future={{ v7_startTransition: true, v7_relativeSplatPath: true }}>
      <Dashboard />
    </MemoryRouter>
  );

  it('should render loading state initially', () => {
    (axios.get as any).mockImplementation(() => new Promise(() => {})); // Never resolves
    const { container } = renderComponent();
    expect(container.querySelectorAll('.skeleton-tile')).toHaveLength(4);
  });

  it('should render stats correctly', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.endsWith('/api/admin/dashboard')) {
        return Promise.resolve({
          data: {
            totalLocations: 10,
            activeSessions: 5,
            totalAttendance: 120,
            flaggedUnreviewed: 3
          }
        });
      }
      if (url.includes('attendance-series')) {
        return Promise.resolve({ data: [] });
      }
      return Promise.resolve({ data: [] });
    });

    renderComponent();
    await waitFor(() => {
      expect(screen.getByText('5')).toBeInTheDocument(); // activeSessions
    });
  });

  it('should render error state on API failure', async () => {
    const err = new Error('API failed');
    (axios.get as any).mockRejectedValue(err);

    renderComponent();
    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith('Failed to fetch stats');
    });
  });
});
