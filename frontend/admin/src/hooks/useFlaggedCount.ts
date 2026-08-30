import { useState, useEffect } from 'react';
import axios from 'axios';

interface FlaggedCountState {
  count: number;
  /// True when at least one unreviewed flag is high-severity. There's no
  /// email/webhook/push alerting in this system — this is the practical
  /// substitute, letting the sidebar badge visually distinguish "something
  /// urgent needs review" from an ordinary backlog.
  hasHighSeverityUnreviewed: boolean;
}

export const useFlaggedCount = (): FlaggedCountState => {
  const [state, setState] = useState<FlaggedCountState>({ count: 0, hasHighSeverityUnreviewed: false });

  useEffect(() => {
    const load = async () => {
      try {
        const res = await axios.get<{
          pulse?: { quarantine?: { count?: number; hasHighSeverityUnreviewed?: boolean } };
        }>('/api/admin/dashboard');
        setState({
          count: res.data.pulse?.quarantine?.count || 0,
          hasHighSeverityUnreviewed: res.data.pulse?.quarantine?.hasHighSeverityUnreviewed || false,
        });
      } catch {
        // silently fail
      }
    };
    load();
    const interval = setInterval(load, 60000);
    return () => clearInterval(interval);
  }, []);

  return state;
};
