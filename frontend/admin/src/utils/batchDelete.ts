export interface DeleteImpact {
  sessionCount: number;
  ruleCount: number;
  activeRuleCount: number;
  /** "archived" when sessions/rules still use the batch, "deleted" when nothing does. */
  mode: 'archived' | 'deleted';
}

/** Spells out, before the admin confirms, what deleting the batch does to
 * the sessions/schedules that use it. */
export const describeDeleteImpact = (name: string, studentCount: number, impact: DeleteImpact | null): string => {
  if (!impact || typeof impact.sessionCount !== 'number') {
    return `Delete "${name}"? If sessions or schedules use it, it is archived instead: hidden from your lists, past attendance and exports are kept, and schedules using it are paused.`;
  }
  if (impact.mode === 'deleted') {
    return `"${name}" and its ${studentCount} student${studentCount === 1 ? '' : 's'} will be permanently deleted. No sessions or schedules use it. This can't be undone.`;
  }
  const parts: string[] = [];
  if (impact.sessionCount > 0) parts.push(`${impact.sessionCount} session${impact.sessionCount === 1 ? '' : 's'}`);
  if (impact.ruleCount > 0) parts.push(`${impact.ruleCount} recurring schedule${impact.ruleCount === 1 ? '' : 's'}`);
  let msg = `"${name}" is used by ${parts.join(' and ')}. It will be removed from your batches list and can't be picked for new sessions. Those existing sessions keep their roster, attendance, absent lists and exports exactly as they are.`;
  if (impact.activeRuleCount > 0) {
    msg += ` ${impact.activeRuleCount} active recurring schedule${impact.activeRuleCount === 1 ? ' that uses' : 's that use'} it will be paused (edit it to choose another batch, then resume).`;
  }
  return `${msg} This can't be undone.`;
};
