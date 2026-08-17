import { api } from './client';
import { AttendanceStatus, ManualAttendanceResponse, RosterResponse, Session } from './types';

export async function fetchSessions(): Promise<Session[]> {
  const res = await api.get<Session[]>('/admin/sessions');
  return res.data;
}

export async function fetchRoster(sessionId: string): Promise<RosterResponse> {
  const res = await api.get<RosterResponse>(`/admin/sessions/${sessionId}/roster`);
  return res.data;
}

export async function markAttendance(
  sessionId: string,
  rollNumber: string,
  status: Extract<AttendanceStatus, 'present' | 'absent'>
): Promise<ManualAttendanceResponse> {
  const res = await api.post<ManualAttendanceResponse>(`/admin/sessions/${sessionId}/attendance/manual`, {
    rollNumber,
    status,
  });
  return res.data;
}

export async function undoAttendance(sessionId: string, rollNumber: string): Promise<void> {
  await api.delete(`/admin/sessions/${sessionId}/attendance/manual/${encodeURIComponent(rollNumber)}`);
}
