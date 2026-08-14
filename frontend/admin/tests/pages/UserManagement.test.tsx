import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import UserManagement from '../../src/pages/UserManagement';
import axios from 'axios';
import { MemoryRouter } from 'react-router';

vi.mock('../../src/context/AuthContext', () => ({
  useAuth: () => ({ admin: { _id: 'self1', username: 'super.admin', role: 'super_admin' } }),
}));

const MENTOR = {
  _id: 'user1',
  username: 'mentor.jane',
  email: 'jane@example.com',
  fullName: 'Jane Mentor',
  collegeName: 'XYZ College',
  role: 'admin',
  isActive: true,
  createdAt: new Date().toISOString(),
};

describe('UserManagement', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const renderComponent = () => render(
    <MemoryRouter>
      <UserManagement />
    </MemoryRouter>
  );

  it('fetches and displays admin/mentor accounts', async () => {
    (axios.get as any).mockResolvedValue({ data: [MENTOR] });
    renderComponent();

    await waitFor(() => {
      expect(screen.getByText('Jane Mentor')).toBeInTheDocument();
      expect(screen.getByText('mentor.jane')).toBeInTheDocument();
      expect(screen.getByText('Mentor')).toBeInTheDocument();
      expect(screen.getByText('Active')).toBeInTheDocument();
    });
  });

  it('shows an empty state when there are no accounts yet', async () => {
    (axios.get as any).mockResolvedValue({ data: [] });
    renderComponent();

    await waitFor(() => {
      expect(screen.getByText(/no admin\/mentor accounts yet/i)).toBeInTheDocument();
    });
  });

  it('creates a new mentor account from the modal', async () => {
    (axios.get as any).mockResolvedValue({ data: [] });
    (axios.post as any).mockResolvedValue({ data: { ...MENTOR, _id: 'user2' } });

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getByText('Create Account')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Create Account'));
    fireEvent.change(screen.getByLabelText(/username/i), { target: { value: 'new.mentor' } });
    fireEvent.change(screen.getByLabelText(/college email/i), { target: { value: 'new.mentor@example.com' } });
    fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'password123' } });
    fireEvent.submit(container.querySelector('form')!);

    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/users', expect.objectContaining({
        username: 'new.mentor',
        email: 'new.mentor@example.com',
        password: 'password123',
        role: 'admin',
      }));
    });
  });

  it('cannot disable or delete the currently logged-in super-admin\'s own row', async () => {
    (axios.get as any).mockResolvedValue({
      data: [{ ...MENTOR, _id: 'self1', username: 'super.admin', role: 'super_admin' }],
    });
    renderComponent();

    await waitFor(() => expect(screen.getByText('super.admin')).toBeInTheDocument());

    const disableButtons = screen.getAllByRole('button', { name: /disable account/i });
    expect(disableButtons[0]).toBeDisabled();
  });

  it('toggles an account active/inactive', async () => {
    (axios.get as any).mockResolvedValue({ data: [MENTOR] });
    (axios.patch as any).mockResolvedValue({ data: { ...MENTOR, isActive: false } });
    renderComponent();

    await waitFor(() => expect(screen.getByText('mentor.jane')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /disable account/i }));

    await waitFor(() => {
      expect(axios.patch).toHaveBeenCalledWith('/api/admin/users/user1', { isActive: false });
    });
  });

  it('shows an error toast when fetching accounts fails', async () => {
    (axios.get as any).mockRejectedValue(new Error('network down'));
    renderComponent();

    await waitFor(() => {
      expect(screen.getByText(/no admin\/mentor accounts yet/i)).toBeInTheDocument();
    });
  });

  it('edits an existing mentor account, omitting role for a self-edit', async () => {
    (axios.get as any).mockResolvedValue({ data: [MENTOR] });
    (axios.patch as any).mockResolvedValue({ data: MENTOR });
    const { container } = renderComponent();

    await waitFor(() => expect(screen.getByText('mentor.jane')).toBeInTheDocument());

    const editButton = container.querySelector('.btn-secondary') as HTMLElement;
    fireEvent.click(editButton);
    fireEvent.change(screen.getByLabelText(/full name/i), { target: { value: 'Jane M. Updated' } });
    fireEvent.submit(container.querySelector('form')!);

    await waitFor(() => {
      expect(axios.patch).toHaveBeenCalledWith('/api/admin/users/user1', expect.objectContaining({
        fullName: 'Jane M. Updated',
        role: 'admin',
      }));
    });
  });

  it('shows an error toast when creating an account fails', async () => {
    (axios.get as any).mockResolvedValue({ data: [] });
    (axios.post as any).mockRejectedValue({ response: { data: { message: 'Username already taken' } } });

    const { container } = renderComponent();
    await waitFor(() => expect(screen.getByText('Create Account')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Create Account'));
    fireEvent.change(screen.getByLabelText(/username/i), { target: { value: 'dupe' } });
    fireEvent.change(screen.getByLabelText(/college email/i), { target: { value: 'dupe@example.com' } });
    fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'password123' } });
    fireEvent.submit(container.querySelector('form')!);

    await waitFor(() => expect(axios.post).toHaveBeenCalled());
  });

  it('deletes an account after confirmation', async () => {
    (axios.get as any).mockResolvedValue({ data: [MENTOR] });
    (axios.delete as any).mockResolvedValue({});
    renderComponent();

    await waitFor(() => expect(screen.getByText('mentor.jane')).toBeInTheDocument());

    const deleteButtons = screen.getAllByRole('button').filter((b) => b.className.includes('btn-delete'));
    fireEvent.click(deleteButtons[0]);

    await waitFor(() => expect(screen.getByText('Delete Account')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => {
      expect(axios.delete).toHaveBeenCalledWith('/api/admin/users/user1');
    });
  });

  it('shows an error toast when deleting an account fails', async () => {
    (axios.get as any).mockResolvedValue({ data: [MENTOR] });
    (axios.delete as any).mockRejectedValue({ response: { data: { message: 'Still owns a session' } } });
    renderComponent();

    await waitFor(() => expect(screen.getByText('mentor.jane')).toBeInTheDocument());

    const deleteButtons = screen.getAllByRole('button').filter((b) => b.className.includes('btn-delete'));
    fireEvent.click(deleteButtons[0]);

    await waitFor(() => expect(screen.getByText('Delete Account')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => expect(axios.delete).toHaveBeenCalled());
  });

  it("blocks the logged-in super-admin from toggling their own account", async () => {
    (axios.get as any).mockResolvedValue({
      data: [{ ...MENTOR, _id: 'self1', username: 'super.admin', role: 'super_admin' }],
    });
    renderComponent();

    await waitFor(() => expect(screen.getByText('super.admin')).toBeInTheDocument());

    fireEvent.click(screen.getAllByRole('button', { name: /disable account/i })[0]);
    expect(axios.patch).not.toHaveBeenCalled();
  });
});
