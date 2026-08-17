import { api } from './client';
import { Admin, LoginResponse } from './types';

export async function loginRequest(username: string, password: string): Promise<LoginResponse> {
  const res = await api.post<LoginResponse>('/admin/login', { username, password });
  return res.data;
}

export async function logoutRequest(): Promise<void> {
  await api.post('/admin/logout');
}

export async function fetchProfile(): Promise<Admin> {
  const res = await api.get<Admin>('/admin/profile');
  return res.data;
}

export async function changePassword(currentPassword: string, newPassword: string): Promise<void> {
  await api.patch('/admin/profile/password', { currentPassword, newPassword });
}
