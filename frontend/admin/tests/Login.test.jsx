import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Login from '../src/pages/Login';

vi.mock('axios', () => ({
  default: {
    get: vi.fn(),
    post: vi.fn(),
    defaults: {
      headers: {
        common: {}
      }
    }
  }
}));

const mockLogin = vi.fn();

vi.mock('../src/context/AuthContext', () => ({
  useAuth: () => ({
    login: mockLogin,
    admin: null,
    loading: false
  })
}));

describe('Login Component Tests', () => {
  beforeEach(() => {
    mockLogin.mockReset();
    localStorage.clear();
  });

  it('should render login form', () => {
    render(<Login />);
    
    expect(screen.getByText('Admin Login')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /login/i })).toBeInTheDocument();
  });

  it('should have text input for username', () => {
    render(<Login />);
    
    const textInputs = screen.getAllByRole('textbox');
    expect(textInputs.length).toBe(1);
  });

  it('should have a submit button', () => {
    render(<Login />);
    
    const submitButton = screen.getByRole('button', { name: /login/i });
    expect(submitButton).toHaveAttribute('type', 'submit');
  });

  it('should have form element', () => {
    render(<Login />);
    
    const form = document.querySelector('form');
    expect(form).toBeTruthy();
  });

  it('should update username on change', () => {
    render(<Login />);
    
    const usernameInput = screen.getByRole('textbox');
    fireEvent.change(usernameInput, { target: { value: 'testadmin' } });
    
    expect(usernameInput).toHaveValue('testadmin');
  });

  it('should have two input fields', () => {
    render(<Login />);
    
    const inputs = document.querySelectorAll('input');
    expect(inputs.length).toBe(2);
  });

  it('should have password input type', () => {
    render(<Login />);
    
    const passwordInput = document.querySelector('input[type="password"]');
    expect(passwordInput).toBeTruthy();
  });

  it('should have required inputs', () => {
    render(<Login />);
    
    const inputs = document.querySelectorAll('input[required]');
    expect(inputs.length).toBe(2);
  });
});
