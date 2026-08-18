import { RosterResponse } from '../api/types';
import { applyLocalStatus } from './useRoster';

const baseRoster: RosterResponse = {
  session: { _id: 'sess1', expiresAt: '2026-08-17T23:00:00Z', manualMarkEarlyWindowMinutes: 30 },
  students: [
    { studentId: '1', rollNumber: 'R1', name: 'Alice', status: 'unmarked', source: null, markedAt: null },
    { studentId: '2', rollNumber: 'R2', name: 'Bob', status: 'unmarked', source: null, markedAt: null },
    { studentId: '3', rollNumber: 'R3', name: 'Cara', status: 'present', source: 'self_submitted', markedAt: '2026-08-17T09:00:00Z' },
  ],
  summary: { total: 3, marked: 1, present: 1, absent: 0, unmarked: 2 },
};

describe('applyLocalStatus', () => {
  it('marks a student present and recomputes the summary counts', () => {
    const result = applyLocalStatus(baseRoster, 'R1', 'present');

    const updated = result.students.find((s) => s.rollNumber === 'R1');
    expect(updated).toMatchObject({ status: 'present', source: 'manual' });
    expect(result.summary).toEqual({ total: 3, marked: 2, present: 2, absent: 0, unmarked: 1 });
  });

  it('marks a student absent and recomputes the summary counts', () => {
    const result = applyLocalStatus(baseRoster, 'R2', 'absent');

    const updated = result.students.find((s) => s.rollNumber === 'R2');
    expect(updated).toMatchObject({ status: 'absent', source: 'manual' });
    expect(result.summary).toEqual({ total: 3, marked: 2, present: 1, absent: 1, unmarked: 1 });
  });

  it('reverting to unmarked clears the source back to null', () => {
    const marked = applyLocalStatus(baseRoster, 'R1', 'present');
    const reverted = applyLocalStatus(marked, 'R1', 'unmarked');

    const updated = reverted.students.find((s) => s.rollNumber === 'R1');
    expect(updated).toMatchObject({ status: 'unmarked', source: null });
    expect(reverted.summary).toEqual(baseRoster.summary);
  });

  it('only touches the targeted student, leaving others untouched', () => {
    const result = applyLocalStatus(baseRoster, 'R1', 'present');

    const untouched = result.students.find((s) => s.rollNumber === 'R3');
    expect(untouched).toEqual(baseRoster.students[2]);
  });

  it('does not mutate the input roster (pure function)', () => {
    const snapshot = JSON.parse(JSON.stringify(baseRoster));
    applyLocalStatus(baseRoster, 'R1', 'present');
    expect(baseRoster).toEqual(snapshot);
  });
});
