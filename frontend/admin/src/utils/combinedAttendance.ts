/**
 * Helpers for the session-detail "All" tab, which for a session that has a
 * roster (batch) shows everyone together: students who checked in AND the
 * roster students who did not (absent). Sessions with no roster have no
 * notion of absence, so "All" stays attendance-only for them.
 */
export interface AbsentRosterStudent {
  name: string;
  rollNumber: string;
  collegeName?: string;
}

/** Absent roster students to append under the attendance rows of the active tab. */
export function absentRowsForTab<T extends AbsentRosterStudent>(
  activeTab: string,
  hasRoster: boolean,
  absentStudents: T[],
): T[] {
  return activeTab === 'all' && hasRoster ? absentStudents : [];
}

/** Count shown on the "All" tab: attendees plus (when a roster exists) absentees. */
export function allTabCount(attendanceCount: number, absentCount: number, hasRoster: boolean): number {
  return hasRoster ? attendanceCount + absentCount : attendanceCount;
}
