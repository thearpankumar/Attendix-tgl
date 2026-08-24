// Event-type strings mirror backend-rust/src/models/telemetry_event.rs's
// doc comment / migrations_timescale/0001_telemetry_events.sql — the
// selected Part A scope from the plan. Kept as a union of string literals,
// not a closed enum on the backend, so a new collector can ship without a
// schema change on either side.
export type TelemetryEventType =
  | 'extension_lifecycle'
  | 'heartbeat'
  | 'idle_state'
  | 'tab_visibility'
  | 'window_focus'
  | 'input_activity'
  | 'tab_url_change'
  | 'tab_count'
  | 'tab_group'
  | 'navigation_history'
  | 'window_count'
  | 'target_tab_focus'
  | 'meet_tab_open'
  | 'device_fingerprint'
  | 'ua_high_entropy'
  | 'ip_webrtc_check'
  | 'emulator_flag'
  | 'multi_monitor';

export interface TelemetryEvent {
  eventType: TelemetryEventType;
  eventData: Record<string, unknown>;
  recordedAt: string; // ISO 8601
}

// Persisted in chrome.storage.local so it survives MV3 service-worker
// restarts — see lib/storage.ts.
export interface PairingState {
  apiBase: string;
  shortCode: string;
  sessionId: string;
  rollNumber: string;
  extensionInstanceId: string;
  targetTabUrlPattern?: string; // e.g. "meet.google.com" — for meet_tab_open/target_tab_focus
  pairedAt: string;
}

export interface PendingPairing {
  apiBase: string;
  shortCode: string;
  pairingCode: string;
  pairingUrl: string;
  extensionInstanceId: string;
}
