import { useState, useEffect, useCallback, useRef } from 'react';
import axios from 'axios';

export interface DeviceLockSummary {
  id: string;
  extensionInstanceId: string;
  status: string;
  lockedAt: string;
  releasedAt?: string;
  supersededBy?: string;
}

export interface TelemetryEventOut {
  eventType: string;
  eventData: Record<string, unknown>;
  recordedAt: string;
}

export interface BehaviorSummary {
  totalIdleSeconds: number;
  focusTransitionCount: number;
  tabSwitchCount: number;
  meetTabEverOpen: boolean;
  eventCount: number;
}

export interface StudentBehaviorResponse {
  rollNumber: string;
  deviceLocks: DeviceLockSummary[];
  summary: BehaviorSummary;
  events: TelemetryEventOut[];
}

/// Shared by the per-row quick-view dialog and the top-panel deep-dive
/// dashboard so both fetch through one place (see backend
/// controllers/student_behavior.rs for the endpoint shapes this mirrors).
/// Pass `live: true` only while the caller actually has the panel open for
/// this student — the WebSocket only connects while `live` is true, matching
/// the backend's "no idle sockets for the other 499 students" design.
export const useStudentBehavior = (
  sessionId: string | undefined,
  rollNumber: string | null,
  live: boolean
) => {
  const [data, setData] = useState<StudentBehaviorResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);

  const fetchBehavior = useCallback(async () => {
    if (!sessionId || !rollNumber) return;
    setLoading(true);
    try {
      const res = await axios.get<StudentBehaviorResponse>(
        `/api/admin/sessions/${sessionId}/students/${rollNumber}/behavior`
      );
      setData(res.data);
    } catch {
      // Leave whatever data is already shown rather than clearing it on a
      // transient fetch failure.
    } finally {
      setLoading(false);
    }
  }, [sessionId, rollNumber]);

  useEffect(() => {
    setData(null);
    if (!sessionId || !rollNumber) return;
    fetchBehavior();
  }, [sessionId, rollNumber, fetchBehavior]);

  useEffect(() => {
    if (!live || !sessionId || !rollNumber) return;

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(
      `${protocol}//${window.location.host}/api/admin/sessions/${sessionId}/students/${rollNumber}/behavior/live`
    );
    wsRef.current = ws;

    // The live message is just a "N new events happened" ping (see
    // controllers::telemetry's Redis publish shape), not full event
    // payloads — so a message just triggers a fresh fetch rather than
    // trying to merge partial data client-side.
    ws.onmessage = () => {
      fetchBehavior();
    };

    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, [live, sessionId, rollNumber, fetchBehavior]);

  return { data, loading, refetch: fetchBehavior };
};
