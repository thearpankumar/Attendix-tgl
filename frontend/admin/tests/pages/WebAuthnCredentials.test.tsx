import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import WebAuthnCredentials from '../../src/pages/WebAuthnCredentials';
import axios from 'axios';

// Regression coverage for the "missing field student_id" bug: the reset/
// suspend/unsuspend POST bodies used to send `{ rollNumber, reason }`, which
// the backend's Rust request structs (requiring `student_id`) rejected
// outright with a deserialization error. These tests pin the payload shape
// so a future regression back to the wrong key is caught here instead of a
// live 400 in production.

const STATS = { totalEnrolled: 1, active: 1, suspended: 0 };
const CREDENTIAL = {
  _id: 'cred1',
  studentId: 'CS101',
  deviceLabel: 'iPhone 15',
  enrolledAt: new Date().toISOString(),
  isSuspended: false,
};

const makeMockGet = (credentials: object[] = [CREDENTIAL]) => (url: string) => {
  if (url.includes('/webauthn/stats')) return Promise.resolve({ data: STATS });
  if (url.includes('/webauthn/credentials')) return Promise.resolve({ data: { credentials, pagination: { pages: 1, total: credentials.length } } });
  return Promise.resolve({ data: {} });
};

describe('WebAuthnCredentials', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('resets a credential with a student_id (not rollNumber) key in the POST body', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    (axios.post as any).mockResolvedValue({ data: { message: 'Credential reset successfully' } });

    render(<WebAuthnCredentials />);
    await waitFor(() => expect(screen.getByText('CS101')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Reset'));
    await waitFor(() => expect(screen.getByText(/Reset Biometric Credential/i)).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/reason for reset/i), { target: { value: 'lost device' } });
    fireEvent.click(screen.getByText('Reset Credential'));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/webauthn/reset', {
        student_id: 'CS101',
        reason: 'lost device',
      });
    });
    // The old (buggy) key must never be sent.
    const [, body] = (axios.post as any).mock.calls[0];
    expect(body).not.toHaveProperty('rollNumber');
  });

  it('suspends a credential with a student_id (not rollNumber) key in the POST body', async () => {
    (axios.get as any).mockImplementation(makeMockGet());
    (axios.post as any).mockResolvedValue({ data: { message: 'ok' } });

    render(<WebAuthnCredentials />);
    await waitFor(() => expect(screen.getByText('CS101')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Suspend'));
    await waitFor(() => expect(screen.getByText(/Suspend Credential/i)).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Enter reason/i), { target: { value: 'suspicious activity' } });
    fireEvent.click(screen.getByText('Suspend', { selector: 'button.btn-primary' }));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/webauthn/suspend', {
        student_id: 'CS101',
        reason: 'suspicious activity',
      });
    });
  });

  it('unsuspends a credential with a student_id (not rollNumber) key in the POST body', async () => {
    (axios.get as any).mockImplementation(makeMockGet([{ ...CREDENTIAL, isSuspended: true }]));
    (axios.post as any).mockResolvedValue({ data: { message: 'ok' } });

    render(<WebAuthnCredentials />);
    await waitFor(() => expect(screen.getByText('CS101')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Unsuspend'));
    await waitFor(() => expect(screen.getByText(/Unsuspend Credential/i)).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Enter reason/i), { target: { value: 'cleared' } });
    fireEvent.click(screen.getByText('Unsuspend', { selector: 'button.btn-primary' }));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/webauthn/unsuspend', {
        student_id: 'CS101',
        reason: 'cleared',
      });
    });
  });
});
