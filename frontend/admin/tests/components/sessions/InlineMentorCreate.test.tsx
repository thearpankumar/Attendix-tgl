import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import axios from 'axios';
import { toast } from 'react-toastify';
import InlineMentorCreate from '../../../src/components/sessions/InlineMentorCreate';

describe('InlineMentorCreate', () => {
  const onCreated = vi.fn();
  const onCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('prefills a username guess from the email local-part', () => {
    render(<InlineMentorCreate email="jane.doe@example.com" onCreated={onCreated} onCancel={onCancel} />);
    expect(screen.getByPlaceholderText('Username')).toHaveValue('janedoe');
  });

  it('creates the account and reports the new mentor back', async () => {
    const created = { _id: 'mentor9', username: 'janedoe', email: 'jane.doe@example.com', fullName: 'Jane Doe', role: 'admin', isActive: true };
    (axios.post as any).mockResolvedValue({ data: created });

    render(<InlineMentorCreate email="jane.doe@example.com" onCreated={onCreated} onCancel={onCancel} />);
    fireEvent.change(screen.getByPlaceholderText(/Full name/i), { target: { value: 'Jane Doe' } });
    fireEvent.change(screen.getByPlaceholderText(/Set a password/i), { target: { value: 'password123' } });
    fireEvent.click(screen.getByRole('button', { name: /Create Account/i }));

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/users', expect.objectContaining({
        username: 'janedoe',
        email: 'jane.doe@example.com',
        fullName: 'Jane Doe',
        password: 'password123',
        role: 'admin',
      }));
    });
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith({
      _id: 'mentor9',
      username: 'janedoe',
      fullName: 'Jane Doe',
      role: 'admin',
      isActive: true,
      email: 'jane.doe@example.com',
    }));
  });

  it('shows an error toast when creation fails', async () => {
    (axios.post as any).mockRejectedValue({ response: { data: { message: 'Email already exists' } } });

    render(<InlineMentorCreate email="dupe@example.com" onCreated={onCreated} onCancel={onCancel} />);
    fireEvent.change(screen.getByPlaceholderText(/Set a password/i), { target: { value: 'password123' } });
    fireEvent.click(screen.getByRole('button', { name: /Create Account/i }));

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Email already exists'));
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('calls onCancel when a cancel button is clicked', () => {
    render(<InlineMentorCreate email="jane.doe@example.com" onCreated={onCreated} onCancel={onCancel} />);
    // Two cancel affordances exist (a header "x" icon and the footer button)
    // — both wire to the same handler.
    fireEvent.click(screen.getAllByRole('button', { name: /^Cancel$/i })[0]);
    expect(onCancel).toHaveBeenCalled();
  });
});
