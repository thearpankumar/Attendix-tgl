import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import axios from 'axios';
import Dashboard from '../../src/pages/Dashboard';
import { MemoryRouter } from 'react-router';
import { mockDashboardData } from '../fixtures/dashboardData';

vi.mock('axios');

const renderDashboard = () =>
  render(
    <MemoryRouter>
      <Dashboard />
    </MemoryRouter>
  );

describe('Dashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(axios.get).mockResolvedValue({ data: mockDashboardData });
  });

  it('renders Command Center title', async () => {
    renderDashboard();
    await waitFor(() => expect(screen.getByText('Command Center')).toBeInTheDocument(), { timeout: 5000 });
  });

  it('shows loading skeleton initially', () => {
    renderDashboard();
    expect(document.querySelector('.animate-pulse')).toBeTruthy();
  });

  it('fetches /api/admin/dashboard on mount', async () => {
    renderDashboard();
    await waitFor(() => expect(axios.get).toHaveBeenCalled(), { timeout: 5000 });
  });

  it('renders widgets after load', async () => {
    renderDashboard();
    await waitFor(() => expect(screen.getByText('Placement Eligibility Index')).toBeInTheDocument(), { timeout: 5000 });
  });

  it('renders Filters button', async () => {
    renderDashboard();
    await waitFor(() => expect(screen.getByText('Filters')).toBeInTheDocument(), { timeout: 5000 });
  });

  it('renders chart sections', async () => {
    renderDashboard();
    await waitFor(() => expect(screen.getByText(/Placement Eligibility Funnel/)).toBeInTheDocument(), { timeout: 5000 });
  });

  it('renders worklist tables', async () => {
    renderDashboard();
    await waitFor(() => expect(screen.getByText(/Placement Rescue List/)).toBeInTheDocument(), { timeout: 5000 });
  });

  it('renders subtitle', async () => {
    renderDashboard();
    await waitFor(() => expect(screen.getByText(/Real-time Insights/)).toBeInTheDocument(), { timeout: 5000 });
  });

  it('hides skeleton after load', async () => {
    renderDashboard();
    await waitFor(() => {
      expect(screen.getByText('Command Center')).toBeInTheDocument();
      expect(document.querySelector('.animate-pulse')).toBeNull();
    }, { timeout: 5000 });
  });
});
