import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';

import { changePassword as changePasswordRequest, loginRequest, logoutRequest } from '../api/auth';
import { apiErrorMessage, setUnauthorizedHandler } from '../api/client';
import { Admin } from '../api/types';
import { clearSession, getCachedAdmin, getToken, saveSession } from '../auth/secureSession';

interface LoginResult {
  success: boolean;
  message?: string;
}

interface AuthContextValue {
  admin: Admin | null;
  loading: boolean;
  login: (username: string, password: string) => Promise<LoginResult>;
  logout: () => Promise<void>;
  changePassword: (currentPassword: string, newPassword: string) => Promise<LoginResult>;
  // True for one render after a fresh password login (not after a biometric
  // unlock) — the biometric-enrollment prompt reads this once, then clears it.
  justAuthenticated: boolean;
  clearJustAuthenticated: () => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export const useAuth = (): AuthContextValue => {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
};

export const AuthProvider = ({ children }: { children: React.ReactNode }) => {
  const [admin, setAdmin] = useState<Admin | null>(null);
  const [loading, setLoading] = useState(true);
  const [justAuthenticated, setJustAuthenticated] = useState(false);

  useEffect(() => {
    (async () => {
      const [token, cached] = await Promise.all([getToken(), getCachedAdmin()]);
      setAdmin(token && cached ? cached : null);
      setLoading(false);
    })();
  }, []);

  // A 401 from any request (expired/blacklisted/revoked token — there is no
  // refresh flow) drops the local session the same way an explicit logout does.
  useEffect(() => {
    setUnauthorizedHandler(() => {
      clearSession();
      setAdmin(null);
    });
    return () => setUnauthorizedHandler(null);
  }, []);

  const login = useCallback(async (username: string, password: string): Promise<LoginResult> => {
    try {
      const res = await loginRequest(username, password);
      if (res.admin.role !== 'admin') {
        // Same backend/login endpoint as super-admin — bounce anyone who
        // isn't a mentor rather than showing a broken mentor UI, and log the
        // token back out immediately so it isn't left dangling on-device.
        await logoutRequest().catch(() => {});
        return { success: false, message: 'This app is for mentors. Super admins should use the main admin panel.' };
      }
      await saveSession(res.token, res.admin);
      setAdmin(res.admin);
      setJustAuthenticated(true);
      return { success: true };
    } catch (error) {
      return { success: false, message: apiErrorMessage(error, 'Login failed') };
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await logoutRequest();
    } catch {
      // Best-effort server-side revocation; local session is always cleared below.
    } finally {
      await clearSession();
      setAdmin(null);
      setJustAuthenticated(false);
    }
  }, []);

  const changePassword = useCallback(async (currentPassword: string, newPassword: string): Promise<LoginResult> => {
    try {
      await changePasswordRequest(currentPassword, newPassword);
      return { success: true };
    } catch (error) {
      return { success: false, message: apiErrorMessage(error, 'Failed to update password') };
    }
  }, []);

  const clearJustAuthenticated = useCallback(() => setJustAuthenticated(false), []);

  const value = useMemo(
    () => ({ admin, loading, login, logout, changePassword, justAuthenticated, clearJustAuthenticated }),
    [admin, loading, login, logout, changePassword, justAuthenticated, clearJustAuthenticated]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
};
