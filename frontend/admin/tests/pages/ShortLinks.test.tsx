import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import ShortLinks from '../../src/pages/ShortLinks';
import axios from 'axios';
import { MemoryRouter } from 'react-router-dom';
import { toast } from 'react-toastify';

describe('ShortLinks', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const renderComponent = () => render(
    <MemoryRouter future={{ v7_startTransition: true, v7_relativeSplatPath: true }}>
      <ShortLinks />
    </MemoryRouter>
  );

  it('should fetch and display short links', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/shortlinks')) {
        return Promise.resolve({
          data: {
            shortLinks: [
              { _id: 'test_link_1', shortCode: 'TEST1', sessionId: null, clickCount: 10, isActive: true, expiresAt: new Date(Date.now() + 10000).toISOString() }
            ]
          }
        });
      }
      if (url.includes('/sessions')) return Promise.resolve({ data: [] });
      return Promise.resolve({ data: [] });
    });

    renderComponent();
    await waitFor(() => {
      expect(screen.getByText('TEST1')).toBeInTheDocument();
      expect(screen.getByText('10')).toBeInTheDocument(); // Click count
    });
  });

  it('should create a new short link', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/shortlinks')) return Promise.resolve({ data: { shortLinks: [] } });
      if (url.includes('/sessions')) return Promise.resolve({ data: [] });
      return Promise.resolve({ data: [] });
    });
    (axios.post as any).mockResolvedValue({ data: { shortCode: 'NEWLINK' } });

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Short Link')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Short Link')[0]);
    
    // Keep auto-generate checked
    const form = container.querySelector('form');
    fireEvent.submit(form!);

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/shortlinks', expect.any(Object));
    });
  });

  it('should handle duplicate short code error', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/shortlinks')) return Promise.resolve({ data: { shortLinks: [] } });
      if (url.includes('/sessions')) return Promise.resolve({ data: [] });
      return Promise.resolve({ data: [] });
    });
    (axios.post as any).mockRejectedValue({ response: { data: { message: 'Short code already in use' } } });

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getAllByText('Create Short Link')[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByText('Create Short Link')[0]);
    
    // Uncheck auto-generate to show the input
    const autoGenCheckbox = screen.getByLabelText(/Auto-generate short code/i);
    fireEvent.click(autoGenCheckbox);

    fireEvent.change(screen.getByLabelText(/Custom Short Code/i), { target: { value: 'DUPE' } });
    
    const form = container.querySelector('form');
    fireEvent.submit(form!);

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith('Short code already in use');
    });
  });
});
