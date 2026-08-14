import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import axios from 'axios';

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
      setAdmin(res.data);
    } catch {
      // If fetching the profile fails (e.g. 401 Unauthorized), we are not logged in.
      // The backend will have already cleared the cookie or it was expired/missing.
      setAdmin(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // With HttpOnly cookies, we always attempt to fetch the profile on load.
    // If we have a valid cookie, it will succeed. If not, it will gracefully 401.
    fetchProfile();
  }, [fetchProfile]);

  const login = useCallback(async (username: string, password: string): Promise<LoginResult> => {
    try {
      const res = await axios.post<{ admin: Admin }>('/api/admin/login', { username, password });
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
      console.warn('Logout request failed', e);
    } finally {
      setAdmin(null);
    }
  }, []);

  const value = React.useMemo(() => ({ admin, login, logout, loading }), [admin, login, logout, loading]);

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
};
