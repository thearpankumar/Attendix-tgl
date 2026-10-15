import { describe, it, expect } from 'vitest';
import { absentRowsForTab, allTabCount } from '../src/utils/combinedAttendance';

const absent = [
  { name: 'A', rollNumber: 'R1' },
  { name: 'B', rollNumber: 'R2' },
];

describe('absentRowsForTab', () => {
  it('shows absentees together with attendees on the All tab when a roster exists', () => {
    expect(absentRowsForTab('all', true, absent)).toEqual(absent);
  });
  it('shows nothing extra on verified/unverified tabs', () => {
    expect(absentRowsForTab('verified', true, absent)).toEqual([]);
    expect(absentRowsForTab('unverified', true, absent)).toEqual([]);
  });
  it('shows nothing for a session without a batch', () => {
    expect(absentRowsForTab('all', false, absent)).toEqual([]);
  });
});

describe('allTabCount', () => {
  it('adds absentees when a roster exists', () => {
    expect(allTabCount(3, 2, true)).toBe(5);
  });
  it('is attendance-only without a roster', () => {
    expect(allTabCount(3, 2, false)).toBe(3);
  });
});
