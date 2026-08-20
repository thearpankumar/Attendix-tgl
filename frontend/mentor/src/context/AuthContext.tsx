import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import axios from 'axios';

import { logWarning } from '../utils/logger';

interface Admin {
  _id: string;
  username: string;
  email: string;
  role: 'admin' | 'super_admin';
  fullName?: string;
  collegeName?: string;
}

interface LoginResult {
  success: boolean;
  message?: string;
}

interface AuthContextValue {
  admin: Admin | null;
  login: (username: string, password: string) => Promise<LoginResult>;
  logout: () => void;
  loading: boolean;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export const useAuth = () => {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
};

export const AuthProvider = ({ children }: { children: React.ReactNode }) => {
  const [admin, setAdmin] = useState<Admin | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchProfile = useCallback(async () => {
    try {
      const res = await axios.get<Admin>('/api/admin/profile');
      // A super-admin's cookie is still valid here (same backend, same
      // login), but this app is mentor-only — bounce them back to the main
      // panel instead of showing a broken mentor UI built for a role they
      // don't have.
      if (res.data.role !== 'admin') {
        setAdmin(null);
      } else {
        setAdmin(res.data);
      }
    } catch {
      setAdmin(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchProfile();
  }, [fetchProfile]);

  const login = useCallback(async (username: string, password: string): Promise<LoginResult> => {
    try {
      const res = await axios.post<{ admin: Admin }>('/api/admin/login', { username, password });
      if (res.data.admin.role !== 'admin') {
        // Wrong app for this account — log the session back out immediately
        // rather than leave a super-admin cookie sitting in a mentor-only app.
        await axios.post('/api/admin/logout').catch(() => {});
        setAdmin(null);
        return { success: false, message: 'This app is for mentors. Super admins should use the main admin panel.' };
      }
      setAdmin(res.data.admin);
      return { success: true };
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      return { success: false, message: err.response?.data?.message || 'Login failed' };
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await axios.post('/api/admin/logout');
    } catch (e) {
      logWarning('Logout request failed', e);
    } finally {
      setAdmin(null);
    }
  }, []);

  const value = React.useMemo(() => ({ admin, login, logout, loading }), [admin, login, logout, loading]);

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
};
