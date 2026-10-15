import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import axios from 'axios';
import { toast } from 'react-toastify';
import Settings from '../../src/pages/Settings';

describe('Settings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const serverConfig = {
    id: '11111111-1111-1111-1111-111111111111',
    devBypassEnabled: false,
    updatedBy: null,
    updatedAt: '2026-01-01T00:00:00Z',
    sessionConfig: { expireMinutes: 60, manualMarkEarlyWindowMinutes: 30, mentorEditWindowHours: 48 },
  };

  it('saves without the read-only fields PUT /api/config rejects (id, devBypassEnabled, updatedBy, updatedAt)', async () => {
    (axios.get as any).mockResolvedValue({ data: serverConfig });
    (axios.put as any).mockResolvedValue({ data: { config: serverConfig } });

    render(<Settings />);
    await waitFor(() => expect(screen.getByText('System Configuration')).toBeInTheDocument());

    // Change one value so the Save button is enabled.
    const inputs = screen.getAllByRole('spinbutton');
    fireEvent.change(inputs[0], { target: { value: '90' } });
    fireEvent.click(screen.getByRole('button', { name: /^Save /i }));

    await waitFor(() => expect(axios.put).toHaveBeenCalled());
    const [url, body] = (axios.put as any).mock.calls[0];
    expect(url).toBe('/api/config');
    for (const key of ['id', 'devBypassEnabled', 'updatedBy', 'updatedAt']) {
      expect(body).not.toHaveProperty(key);
    }
    expect(body.sessionConfig.expireMinutes).toBe(90);
  });

  it('shows the server message when a save is rejected (e.g. not a super-admin)', async () => {
    (axios.get as any).mockResolvedValue({ data: serverConfig });
    (axios.put as any).mockRejectedValue({ isAxiosError: true, response: { data: { message: 'Insufficient permissions' } } });
    (axios.isAxiosError as any) = () => true;

    render(<Settings />);
    await waitFor(() => expect(screen.getByText('System Configuration')).toBeInTheDocument());
    fireEvent.change(screen.getAllByRole('spinbutton')[0], { target: { value: '90' } });
    fireEvent.click(screen.getByRole('button', { name: /^Save /i }));

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Insufficient permissions'));
  });
});
