import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import axios from 'axios';
import { useFlaggedCount } from '../../src/hooks/useFlaggedCount';

describe('useFlaggedCount', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  // Regression test: the backend's dashboard response has no top-level
  // `flaggedUnreviewed` field — the count of flagged-and-unverified
  // submissions lives at `pulse.quarantine.count`
  // (backend-rust/src/controllers/admin/dashboard.rs). Reading the wrong
  // path silently always renders a 0 badge.
  it('reads the count from pulse.quarantine.count', async () => {
    (axios.get as any).mockResolvedValueOnce({
      data: { pulse: { quarantine: { count: 7 } } },
    });

    const { result } = renderHook(() => useFlaggedCount());

    await waitFor(() => expect(result.current).toBe(7));
    expect(axios.get).toHaveBeenCalledWith('/api/admin/dashboard');
  });

  it('defaults to 0 when quarantine data is missing', async () => {
    (axios.get as any).mockResolvedValueOnce({ data: {} });

    const { result } = renderHook(() => useFlaggedCount());

    await waitFor(() => expect(axios.get).toHaveBeenCalled());
    expect(result.current).toBe(0);
  });

  it('defaults to 0 and does not throw when the request fails', async () => {
    (axios.get as any).mockRejectedValueOnce(new Error('network error'));

    const { result } = renderHook(() => useFlaggedCount());

    await waitFor(() => expect(axios.get).toHaveBeenCalled());
    expect(result.current).toBe(0);
  });
});
