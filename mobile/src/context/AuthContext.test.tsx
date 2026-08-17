/* eslint-disable import/first -- jest.mock must precede the imports it mocks */
jest.mock('../api/auth', () => ({
  loginRequest: jest.fn(),
  logoutRequest: jest.fn(),
  fetchProfile: jest.fn(),
  changePassword: jest.fn(),
}));

jest.mock('../auth/secureSession', () => ({
  saveSession: jest.fn(),
  getCachedAdmin: jest.fn(),
  getToken: jest.fn(),
  clearSession: jest.fn(),
}));

import { act, renderHook, waitFor } from '@testing-library/react-native';
import React from 'react';

import { changePassword as changePasswordRequest, loginRequest, logoutRequest } from '../api/auth';
import { Admin, LoginResponse } from '../api/types';
import { clearSession, getCachedAdmin, getToken, saveSession } from '../auth/secureSession';
import { AuthProvider, useAuth } from './AuthContext';

const mockedLogin = loginRequest as jest.MockedFunction<() => Promise<LoginResponse>>;
const mockedLogout = logoutRequest as jest.MockedFunction<() => Promise<void>>;
const mockedChangePassword = changePasswordRequest as jest.Mock;
const mockedGetToken = getToken as jest.Mock;
const mockedGetCachedAdmin = getCachedAdmin as jest.Mock;
const mockedSaveSession = saveSession as jest.Mock;
const mockedClearSession = clearSession as jest.Mock;

const admin: Admin = { _id: '1', username: 'mentor1', email: 'm@example.com', role: 'admin' };
const superAdmin: Admin = { ...admin, role: 'super_admin' };

const wrapper = ({ children }: { children: React.ReactNode }) => <AuthProvider>{children}</AuthProvider>;

beforeEach(() => {
  jest.clearAllMocks();
  mockedGetToken.mockResolvedValue(null);
  mockedGetCachedAdmin.mockResolvedValue(null);
  mockedLogout.mockResolvedValue(undefined);
});

async function readyHook() {
  const { result } = await renderHook(() => useAuth(), { wrapper });
  await waitFor(() => expect(result.current.loading).toBe(false));
  return result;
}

describe('AuthProvider bootstrap', () => {
  it('resolves to no admin when nothing is stored on disk', async () => {
    const result = await readyHook();
    expect(result.current.admin).toBeNull();
  });

  it('restores a cached admin when both a token and a cached profile exist', async () => {
    mockedGetToken.mockResolvedValue('jwt-abc');
    mockedGetCachedAdmin.mockResolvedValue(admin);

    const result = await readyHook();
    expect(result.current.admin).toEqual(admin);
  });

  it('does not restore a cached admin if only the profile exists without a token', async () => {
    mockedGetToken.mockResolvedValue(null);
    mockedGetCachedAdmin.mockResolvedValue(admin);

    const result = await readyHook();
    expect(result.current.admin).toBeNull();
  });
});

describe('login', () => {
  it('persists the session and sets admin on a successful mentor login', async () => {
    mockedLogin.mockResolvedValue({ token: 'jwt-abc', expires_in: '7d', admin });
    const result = await readyHook();

    let outcome: { success: boolean; message?: string } | undefined;
    await act(async () => {
      outcome = await result.current.login('mentor1', 'pw');
    });

    expect(outcome).toEqual({ success: true });
    expect(mockedSaveSession).toHaveBeenCalledWith('jwt-abc', admin);
    expect(result.current.admin).toEqual(admin);
    expect(result.current.justAuthenticated).toBe(true);
  });

  it('bounces a super_admin login: logs the token back out server-side and never sets admin locally', async () => {
    mockedLogin.mockResolvedValue({ token: 'jwt-abc', expires_in: '7d', admin: superAdmin });
    const result = await readyHook();

    let outcome: { success: boolean; message?: string } | undefined;
    await act(async () => {
      outcome = await result.current.login('super', 'pw');
    });

    expect(outcome?.success).toBe(false);
    expect(outcome?.message).toMatch(/mentors/i);
    expect(mockedLogout).toHaveBeenCalled();
    expect(mockedSaveSession).not.toHaveBeenCalled();
    expect(result.current.admin).toBeNull();
  });

  it('surfaces the server error message on a failed login', async () => {
    mockedLogin.mockRejectedValue({ response: { data: { message: 'Invalid credentials' } } });
    const result = await readyHook();

    let outcome: { success: boolean; message?: string } | undefined;
    await act(async () => {
      outcome = await result.current.login('mentor1', 'wrong');
    });

    expect(outcome).toEqual({ success: false, message: 'Invalid credentials' });
    expect(result.current.admin).toBeNull();
  });
});

describe('logout', () => {
  it('clears the stored session and resets admin/justAuthenticated', async () => {
    mockedLogin.mockResolvedValue({ token: 'jwt-abc', expires_in: '7d', admin });
    mockedLogout.mockResolvedValue(undefined);
    const result = await readyHook();

    await act(async () => {
      await result.current.login('mentor1', 'pw');
    });
    expect(result.current.admin).toEqual(admin);

    await act(async () => {
      await result.current.logout();
    });

    expect(mockedClearSession).toHaveBeenCalled();
    expect(result.current.admin).toBeNull();
    expect(result.current.justAuthenticated).toBe(false);
  });

  it('still clears the local session even if the server-side logout call fails', async () => {
    mockedLogout.mockRejectedValue(new Error('network error'));
    const result = await readyHook();

    await act(async () => {
      await result.current.logout();
    });

    expect(mockedClearSession).toHaveBeenCalled();
    expect(result.current.admin).toBeNull();
  });
});

describe('changePassword', () => {
  it('returns success when the request succeeds', async () => {
    mockedChangePassword.mockResolvedValue(undefined);
    const result = await readyHook();

    let outcome: { success: boolean; message?: string } | undefined;
    await act(async () => {
      outcome = await result.current.changePassword('old', 'newpass');
    });

    expect(outcome).toEqual({ success: true });
    expect(mockedChangePassword).toHaveBeenCalledWith('old', 'newpass');
  });

  it('surfaces the server error message on failure', async () => {
    mockedChangePassword.mockRejectedValue({ response: { data: { message: 'Current password is incorrect' } } });
    const result = await readyHook();

    let outcome: { success: boolean; message?: string } | undefined;
    await act(async () => {
      outcome = await result.current.changePassword('wrong', 'newpass');
    });

    expect(outcome).toEqual({ success: false, message: 'Current password is incorrect' });
  });
});
