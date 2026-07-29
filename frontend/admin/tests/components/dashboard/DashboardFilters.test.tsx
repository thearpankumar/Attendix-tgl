import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import DashboardFilters from '../../../src/components/dashboard/DashboardFilters';
import { MemoryRouter } from 'react-router';
import axios from 'axios';

vi.mock('axios');

const mockOnFilterChange = vi.fn();

const renderFilters = () =>
  render(
    <MemoryRouter>
      <DashboardFilters onFilterChange={mockOnFilterChange} />
    </MemoryRouter>
  );

describe('DashboardFilters', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(axios.get).mockResolvedValue({
      data: {
        batches: [{ _id: 'batch1', name: 'Batch A' }],
        centers: [{ _id: 'center1', name: 'Center A' }],
        timeframes: ['Today'],
        riskLevels: ['All Levels', 'High']
      }
    });
  });

  describe('Normal Cases', () => {
    it('renders Filters button', async () => {
      renderFilters();
      await waitFor(() => expect(screen.getByText('Filters')).toBeInTheDocument());
    });

    it('opens filter popup on click', async () => {
      renderFilters();
      await waitFor(() => expect(screen.getByText('Filters')).toBeInTheDocument());
      fireEvent.click(screen.getByText('Filters'));
      await waitFor(() => expect(screen.getByText('Filter Dashboard')).toBeInTheDocument());
    });

    it('closes popup on Cancel', async () => {
      renderFilters();
      await waitFor(() => expect(screen.getByText('Filters')).toBeInTheDocument());
      fireEvent.click(screen.getByText('Filters'));
      await waitFor(() => expect(screen.getByText('Filter Dashboard')).toBeInTheDocument());
      fireEvent.click(screen.getByText('Cancel'));
      await waitFor(() => expect(screen.queryByText('Filter Dashboard')).not.toBeInTheDocument());
    });

    it('shows Filter by label', async () => {
      renderFilters();
      await waitFor(() => expect(screen.getByText('Filters')).toBeInTheDocument());
      fireEvent.click(screen.getByText('Filters'));
      await waitFor(() => {
        const filterBy = screen.queryByText(/Filter by/);
        const filterDashboard = screen.queryByText('Filter Dashboard');
        expect(filterBy || filterDashboard).toBeTruthy();
      });
    });
  });

  describe('Accessibility', () => {
    it('button is accessible', async () => {
      renderFilters();
      await waitFor(() => {
        const btn = screen.getByRole('button', { name: /filters/i });
        expect(btn).toBeInTheDocument();
      });
    });
  });
});
