export function formatSlot(iso?: string): string | null {
  if (!iso) return null;
  return new Date(iso).toLocaleString(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function formatClock(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}

export function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function timeUntil(iso: string): string {
  const diffMin = Math.round((new Date(iso).getTime() - Date.now()) / 60000);
  if (diffMin <= 0) return 'Starting now';
  const h = Math.floor(diffMin / 60);
  const m = diffMin % 60;
  return h > 0 ? `Starts in ${h}h ${m}m` : `Starts in ${m}m`;
}
