/**
 * Shared attendance-percentage color thresholds so the batch Student View
 * and the global Student Lookup page render identical colors for the same
 * number, rather than each page inlining its own bucketing.
 */
export function attendanceColor(percentage: number): string {
  if (percentage >= 85) return 'var(--color-success)';
  if (percentage >= 60) return 'var(--color-warning)';
  return 'var(--color-danger)';
}

/** Tinted pill background to pair with `attendanceColor` (e.g. `.percent-pill`). */
export function attendanceColorBg(percentage: number): string {
  return `color-mix(in srgb, ${attendanceColor(percentage)} 16%, var(--color-surface))`;
}
