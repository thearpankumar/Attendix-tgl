import { formatDayMonthTime, formatTime, formatWeekdayDateTime } from './datetime';

/** "Fri, 16 Oct, 03:19 PM" (IST). */
export function formatSlot(iso?: string): string | null {
  if (!iso) return null;
  return formatWeekdayDateTime(iso);
}

/** "03:19 PM" (IST). */
export function formatClock(iso: string): string {
  return formatTime(iso);
}

/** "16 Oct, 03:19 PM" (IST). */
export function formatDateTime(iso: string): string {
  return formatDayMonthTime(iso);
}

export function timeUntil(iso: string): string {
  const diffMin = Math.round((new Date(iso).getTime() - Date.now()) / 60000);
  if (diffMin <= 0) return 'Starting now';
  const h = Math.floor(diffMin / 60);
  const m = diffMin % 60;
  return h > 0 ? `Starts in ${h}h ${m}m` : `Starts in ${m}m`;
}
