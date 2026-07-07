import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Sessions from '../../src/pages/Sessions';
import axios from 'axios';
import { MemoryRouter } from 'react-router-dom';

describe('Sessions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const renderComponent = () => render(
    <MemoryRouter future={{ v7_startTransition: true, v7_relativeSplatPath: true }}>
      <Sessions />
    </MemoryRouter>
  );

  it('should render sessions list', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/sessions')) {
        return Promise.resolve({
          data: [
            { _id: '1', locationId: { name: 'Room 101' }, isActive: true, expiresAt: new Date(Date.now() + 10000).toISOString(), createdAt: new Date().toISOString() }
          ]
        });
      }
      if (url.includes('/locations')) return Promise.resolve({ data: [{ _id: 'loc1', name: 'Room 101' }] });
      return Promise.resolve({ data: [] });
    });

    renderComponent();
    await waitFor(() => {
      expect(screen.getByText('Room 101')).toBeInTheDocument();
      expect(screen.getByText(/Active/i)).toBeInTheDocument();
    });
  });

  it('should render empty state if no sessions', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/sessions')) return Promise.resolve({ data: [] });
      if (url.includes('/locations')) return Promise.resolve({ data: [{ _id: 'loc1', name: 'Room 101' }] });
      return Promise.resolve({ data: [] });
    });
    
    renderComponent();
    await waitFor(() => {
      expect(screen.getByText(/No sessions yet/i)).toBeInTheDocument();
    });
  });

  it('should toggle create session modal', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/sessions')) return Promise.resolve({ data: [] });
      if (url.includes('/locations')) return Promise.resolve({ data: [{ _id: 'loc1', name: 'Room 101' }] });
      return Promise.resolve({ data: [] });
    });
    
    renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Session')[0]).toBeInTheDocument());
    
    fireEvent.click(screen.getAllByText('Create Session')[0]);
    expect(screen.getByText(/Select a location/i)).toBeInTheDocument();
    
    fireEvent.click(screen.getByText(/Cancel/i));
    await waitFor(() => {
      expect(screen.queryByText(/Select a location/i)).not.toBeInTheDocument();
    });
  });
});
