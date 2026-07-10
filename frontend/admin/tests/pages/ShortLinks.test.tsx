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

  it('should handle detach short link', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/shortlinks')) return Promise.resolve({ data: { shortLinks: [{ _id: '1', shortCode: 'TEST1', sessionId: { _id: 'sess1' }, isActive: true, createdAt: new Date().toISOString() }] } });
      if (url.includes('/sessions')) return Promise.resolve({ data: [] });
      return Promise.resolve({ data: [] });
    });
    (axios.post as any).mockResolvedValue({});
    
    renderComponent();
    await waitFor(() => expect(screen.getAllByText('Detach')[0]).toBeInTheDocument());
    
    fireEvent.click(screen.getAllByText('Detach')[0]);
    await waitFor(() => expect(screen.getByRole('heading', { name: /Detach Short Link/i })).toBeInTheDocument());
    
    // The modal has a Detach button
    const modalDetachBtn = screen.getAllByText('Detach')[1]; // The second one is in the modal
    fireEvent.click(modalDetachBtn);

    await waitFor(() => expect(axios.post).toHaveBeenCalledWith('/api/admin/shortlinks/TEST1/detach'));
  });

  it('should handle attach short link', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/shortlinks')) return Promise.resolve({ data: { shortLinks: [{ _id: '1', shortCode: 'TEST1', sessionId: null, isActive: true, createdAt: new Date().toISOString() }] } });
      if (url.includes('/sessions')) return Promise.resolve({ data: [{ _id: 'sess1', isActive: true, expiresAt: new Date(Date.now() + 100000).toISOString(), description: 'Active Session' }] });
      return Promise.resolve({ data: [] });
    });
    (axios.post as any).mockResolvedValue({});

    renderComponent();
    await waitFor(() => expect(screen.getByRole('combobox')).toBeInTheDocument());
    
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'sess1' } });
    await waitFor(() => expect(axios.post).toHaveBeenCalledWith('/api/admin/shortlinks/TEST1/attach', { sessionId: 'sess1' }));
  });

  it('should handle delete short link', async () => {
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/shortlinks')) return Promise.resolve({ data: { shortLinks: [{ _id: '1', shortCode: 'TEST1', isActive: true, createdAt: new Date().toISOString() }] } });
      if (url.includes('/sessions')) return Promise.resolve({ data: [] });
      return Promise.resolve({ data: [] });
    });
    (axios.delete as any).mockResolvedValue({});
    
    renderComponent();
    await waitFor(() => expect(screen.getAllByText('Delete')[0]).toBeInTheDocument());
    
    fireEvent.click(screen.getAllByText('Delete')[0]);
    await waitFor(() => expect(screen.getByRole('heading', { name: /Delete Short Link/i })).toBeInTheDocument());
    
    // The modal has a Delete button
    const modalDeleteBtn = screen.getAllByText('Delete')[1]; // The second one is in the modal
    fireEvent.click(modalDeleteBtn);
    
    await waitFor(() => expect(axios.delete).toHaveBeenCalledWith('/api/admin/shortlinks/TEST1'));
  });
});
