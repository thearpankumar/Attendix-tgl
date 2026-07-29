import { useState, useEffect } from 'react';
import axios from 'axios';

export const useFlaggedCount = (): number => {
  const [count, setCount] = useState(0);

  useEffect(() => {
    const load = async () => {
      try {
        const res = await axios.get<{ pulse?: { quarantine?: { count?: number } } }>('/api/admin/dashboard');
        setCount(res.data.pulse?.quarantine?.count || 0);
      } catch {
        // silently fail
      }
    };
    load();
    const interval = setInterval(load, 60000);
    return () => clearInterval(interval);
  }, []);

  return count;
};
