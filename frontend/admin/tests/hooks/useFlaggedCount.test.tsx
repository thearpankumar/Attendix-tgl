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
      data: { pulse: { quarantine: { count: 7, hasHighSeverityUnreviewed: false } } },
    });

    const { result } = renderHook(() => useFlaggedCount());

    await waitFor(() => expect(result.current.count).toBe(7));
    expect(axios.get).toHaveBeenCalledWith('/api/admin/dashboard');
  });

  // The dashboard's quarantine metric doubles as this system's substitute
  // for real alerting (no email/webhook/push infra exists) — the badge
  // should be able to tell "urgent" apart from "just a backlog".
  it('reads hasHighSeverityUnreviewed from pulse.quarantine', async () => {
    (axios.get as any).mockResolvedValueOnce({
      data: { pulse: { quarantine: { count: 3, hasHighSeverityUnreviewed: true } } },
    });

    const { result } = renderHook(() => useFlaggedCount());

    await waitFor(() => expect(result.current.hasHighSeverityUnreviewed).toBe(true));
  });

  it('defaults to 0/false when quarantine data is missing', async () => {
    (axios.get as any).mockResolvedValueOnce({ data: {} });

    const { result } = renderHook(() => useFlaggedCount());

    await waitFor(() => expect(axios.get).toHaveBeenCalled());
    expect(result.current.count).toBe(0);
    expect(result.current.hasHighSeverityUnreviewed).toBe(false);
  });

  it('defaults to 0/false and does not throw when the request fails', async () => {
    (axios.get as any).mockRejectedValueOnce(new Error('network error'));

    const { result } = renderHook(() => useFlaggedCount());

    await waitFor(() => expect(axios.get).toHaveBeenCalled());
    expect(result.current.count).toBe(0);
    expect(result.current.hasHighSeverityUnreviewed).toBe(false);
  });
});
