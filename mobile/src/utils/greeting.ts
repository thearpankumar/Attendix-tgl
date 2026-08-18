export function greetingWord(): string {
  const hour = new Date().getHours();
  if (hour < 12) return 'Good morning';
  if (hour < 17) return 'Good afternoon';
  return 'Good evening';
}

export type GreetingPeriod = 'morning' | 'afternoon' | 'night';

// Same hour boundaries as greetingWord — "evening" shares the "night" visual
// treatment since both cover the same after-17:00 stretch of the day.
export function greetingPeriod(): GreetingPeriod {
  const hour = new Date().getHours();
  if (hour < 12) return 'morning';
  if (hour < 17) return 'afternoon';
  return 'night';
}
