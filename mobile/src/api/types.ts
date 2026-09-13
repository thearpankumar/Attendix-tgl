export interface Admin {
  _id: string;
  username: string;
  email: string;
  role: 'admin' | 'super_admin';
  fullName?: string;
  collegeName?: string;
}

export interface LoginResponse {
  token: string;
  expires_in: string;
  admin: Admin;
}

export interface Session {
  _id: string;
  locationId?: string;
  locationName?: string;
  batchId?: string;
  batchName?: string;
  description?: string;
  isActive: boolean;
  expiresAt: string;
  startsAt?: string;
  tokenPrefix?: string;
  createdAt?: string;
  attendanceCount: number;
  shortCode?: string;
  assignedAdminIds?: string[];
  assignedAdminNames?: string[];
  collegeName?: string;
  // Normal (non-exam) sessions only — how many hours after createdAt a
  // mentor may still be added/removed and may still mark/undo attendance.
  mentorEditWindowHours?: number;
}

export type AttendanceStatus = 'unmarked' | 'present' | 'absent';
export type AttendanceSource = 'self_submitted' | 'manual' | null;

export interface RosterStudent {
  studentId: string;
  rollNumber: string;
  name: string;
  collegeName?: string;
  email?: string;
  status: AttendanceStatus;
  source: AttendanceSource;
  markedAt: string | null;
}

export interface RosterSummary {
  total: number;
  marked: number;
  present: number;
  absent: number;
  unmarked: number;
}

export interface RosterResponse {
  session: {
    _id: string;
    collegeName?: string;
    startsAt?: string;
    expiresAt: string;
    createdAt: string;
    batchName?: string;
    description?: string;
    // Super-admin configurable (Settings page) — how many minutes before
    // startsAt a mentor may begin manually marking attendance.
    manualMarkEarlyWindowMinutes: number;
    // Normal (non-exam) sessions only — how many hours after createdAt a
    // mentor may still mark/undo attendance, even after the session's own
    // (often much shorter) self-check-in window has closed.
    mentorEditWindowHours: number;
  };
  students: RosterStudent[];
  summary: RosterSummary;
}

export interface ManualAttendanceResponse {
  success: boolean;
  attendanceId: string;
  status: AttendanceStatus;
  rollNumber: string;
}

export interface ApiErrorBody {
  success?: boolean;
  error?: string;
  message?: string;
}
