import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { fetchRoster, markAttendance, undoAttendance } from '../api/sessions';
import { AttendanceStatus, RosterResponse, RosterStudent } from '../api/types';

export function useRoster(sessionId: string) {
  return useQuery({
    queryKey: ['roster', sessionId],
    queryFn: () => fetchRoster(sessionId),
    enabled: !!sessionId,
  });
}

// Exported for direct unit testing — the optimistic-update math is worth
// covering in isolation rather than only indirectly through the mutation hooks.
export function applyLocalStatus(data: RosterResponse, rollNumber: string, status: AttendanceStatus): RosterResponse {
  const students = data.students.map((s) =>
    s.rollNumber === rollNumber ? { ...s, status, source: status === 'unmarked' ? null : ('manual' as const) } : s
  );
  const present = students.filter((s) => s.status === 'present').length;
  const absent = students.filter((s) => s.status === 'absent').length;
  const marked = present + absent;
  return { ...data, students, summary: { ...data.summary, present, absent, marked, unmarked: data.summary.total - marked } };
}

interface MutationContext {
  previous?: RosterResponse;
}

export function useMarkAttendance(sessionId: string) {
  const queryClient = useQueryClient();
  const key = ['roster', sessionId];

  return useMutation<unknown, unknown, { student: RosterStudent; status: 'present' | 'absent' }, MutationContext>({
    mutationFn: ({ student, status }) => markAttendance(sessionId, student.rollNumber, status),
    onMutate: async ({ student, status }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<RosterResponse>(key);
      if (previous) queryClient.setQueryData<RosterResponse>(key, applyLocalStatus(previous, student.rollNumber, status));
      return { previous };
    },
    onError: (_err, _vars, context) => {
      // Covers both genuine failures and the expected 409 (student already
      // self-submitted) — either way, roll back and let the refetch below
      // bring in the server's real (read-only, self_submitted) row state.
      if (context?.previous) queryClient.setQueryData(key, context.previous);
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: key });
    },
  });
}

export function useUndoAttendance(sessionId: string) {
  const queryClient = useQueryClient();
  const key = ['roster', sessionId];

  return useMutation<unknown, unknown, RosterStudent, MutationContext>({
    mutationFn: (student) => undoAttendance(sessionId, student.rollNumber),
    onMutate: async (student) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<RosterResponse>(key);
      if (previous) queryClient.setQueryData<RosterResponse>(key, applyLocalStatus(previous, student.rollNumber, 'unmarked'));
      return { previous };
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) queryClient.setQueryData(key, context.previous);
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: key });
    },
  });
}
