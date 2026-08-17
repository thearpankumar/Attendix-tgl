import { useQuery } from '@tanstack/react-query';

import { fetchRoster, fetchSessions } from '../api/sessions';
import { RosterSummary, Session } from '../api/types';

export type SessionState = 'live' | 'upcoming' | 'closed';

export function sessionState(s: Session): SessionState {
  const now = Date.now();
  if (!s.isActive || new Date(s.expiresAt).getTime() < now) return 'closed';
  if (s.startsAt && new Date(s.startsAt).getTime() > now) return 'upcoming';
  return 'live';
}

export interface DashboardData {
  sessions: Session[];
  rosterMap: Record<string, RosterSummary>;
}

async function fetchDashboard(): Promise<DashboardData> {
  const sessions = await fetchSessions();
  const open = sessions.filter((s) => sessionState(s) !== 'closed');

  const entries = await Promise.all(
    open.map(async (s) => {
      try {
        const roster = await fetchRoster(s._id);
        return [s._id, roster.summary] as const;
      } catch {
        return [s._id, null] as const;
      }
    })
  );

  const rosterMap = Object.fromEntries(
    entries.filter((entry): entry is [string, RosterSummary] => entry[1] !== null)
  );

  return { sessions, rosterMap };
}

export function useDashboard() {
  return useQuery({
    queryKey: ['dashboard'],
    queryFn: fetchDashboard,
    refetchInterval: 30_000,
  });
}
