import type { TelemetryEvent } from './types';

/** Thin fetch wrappers for the Phase 2/3 backend endpoints this extension
 * talks to. All of them live under `{apiBase}/s/{shortCode}/extension/...`
 * — see backend-rust/src/routes/short_link.rs's `telemetry_routes`. */

export interface StartPairingResponse {
  pairingCode: string;
  pairingUrl: string;
}

export async function startPairing(
  apiBase: string,
  shortCode: string,
  extensionInstanceId: string,
): Promise<StartPairingResponse> {
  const res = await fetch(`${apiBase}/s/${shortCode}/extension/pair/start`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ extensionInstanceId }),
  });
  if (!res.ok) throw new Error(`Failed to start pairing (${res.status})`);
  return res.json();
}

export interface PairingStatusResponse {
  status: 'pending' | 'completed' | 'expired';
  rollNumber?: string;
}

export async function pollPairingStatus(
  apiBase: string,
  shortCode: string,
  pairingCode: string,
): Promise<PairingStatusResponse> {
  const res = await fetch(`${apiBase}/s/${shortCode}/extension/pair/status/${pairingCode}`);
  if (!res.ok) throw new Error(`Failed to poll pairing status (${res.status})`);
  return res.json();
}

export interface FinishPairingResponse {
  locked: boolean;
  rollNumber: string;
}

export async function finishPairing(
  apiBase: string,
  shortCode: string,
  pairingCode: string,
  deviceFingerprintHash: string,
  browserName: string,
  os: string,
): Promise<FinishPairingResponse> {
  const res = await fetch(`${apiBase}/s/${shortCode}/extension/pair/finish`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      pairingCode,
      deviceFingerprintHash,
      browser: browserName,
      os,
    }),
  });
  if (!res.ok) throw new Error(`Failed to finish pairing (${res.status})`);
  return res.json();
}

/** Returns `false` on a 403 specifically (device lock lost/superseded) so
 * the caller can distinguish "stop trying, re-pair" from a transient
 * network failure worth retrying. */
export async function postEvents(
  apiBase: string,
  shortCode: string,
  extensionInstanceId: string,
  rollNumber: string,
  events: TelemetryEvent[],
): Promise<boolean> {
  const res = await fetch(`${apiBase}/s/${shortCode}/extension/events`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ extensionInstanceId, rollNumber, events }),
  });
  if (res.status === 403) return false;
  if (!res.ok) throw new Error(`Failed to post telemetry batch (${res.status})`);
  return true;
}
