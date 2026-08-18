// Simple dotted-numeric comparison for three-part app versions (e.g. "1.2.10"
// vs "1.2.9") — no semver library needed for a version scheme this narrow.
export function isNewerVersion(latest: string, current: string): boolean {
  const latestParts = latest.split('.').map((part) => parseInt(part, 10) || 0);
  const currentParts = current.split('.').map((part) => parseInt(part, 10) || 0);
  const length = Math.max(latestParts.length, currentParts.length);

  for (let i = 0; i < length; i++) {
    const l = latestParts[i] ?? 0;
    const c = currentParts[i] ?? 0;
    if (l !== c) return l > c;
  }
  return false;
}
