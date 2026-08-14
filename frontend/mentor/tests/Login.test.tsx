import { render, screen, fireEvent, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { MemoryRouter } from 'react-router';
import Login from '../src/pages/Login';

const mockLogin = vi.fn();

vi.mock('../src/context/AuthContext', () => ({
  useAuth: () => ({
    login: mockLogin,
    admin: null,
    loading: false,
  }),
}));

describe('Mentor Login', () => {
  beforeEach(() => {
    mockLogin.mockReset();
  });

  const renderLogin = () => render(
    <MemoryRouter>
      <Login />
    </MemoryRouter>
  );

  it('renders the mentor branding and form', () => {
    renderLogin();
    expect(screen.getByText('Exam Attendance Portal')).toBeInTheDocument();
    expect(screen.getByText(/please login to continue/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/username/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/password/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign in/i })).toBeInTheDocument();
  });

  it('requires both fields', () => {
    renderLogin();
    const inputs = document.querySelectorAll('input[required]');
    expect(inputs.length).toBe(2);
  });

  it('calls login with the entered credentials on submit', async () => {
    mockLogin.mockResolvedValue({ success: true });
    const { container } = renderLogin();

    fireEvent.change(screen.getByLabelText(/username/i), { target: { value: 'mentor.jane' } });
    fireEvent.change(screen.getByLabelText(/password/i), { target: { value: 'secret123' } });

    await act(async () => {
      fireEvent.submit(container.querySelector('form')!);
    });

    expect(mockLogin).toHaveBeenCalledWith('mentor.jane', 'secret123');
  });

  it('shows the error message returned by a failed login', async () => {
    mockLogin.mockResolvedValue({ success: false, message: 'This app is for mentors — admins, use the main panel.' });
    const { container } = renderLogin();

    await act(async () => {
      fireEvent.submit(container.querySelector('form')!);
    });

    expect(await screen.findByText(/this app is for mentors/i)).toBeInTheDocument();
  });
});
