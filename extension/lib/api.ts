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
  telemetryToken: string;
  telemetryTokenExpiresAt: string;
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

/** Returns `false` on a 403 specifically (device lock lost/superseded, or a
 * telemetry token that's expired/blacklisted/stale) so the caller can
 * distinguish "stop trying, re-pair" from a transient network failure worth
 * retrying. `telemetryToken` is sent as `Authorization: Bearer` — see
 * backend-rust/src/controllers/telemetry.rs::ingest_telemetry's dual-mode
 * doc comment; `extensionInstanceId`/`rollNumber` stay in the body for the
 * legacy fallback path (harmless once the signed-token path is in use, since
 * the backend prefers the header when present). */
export async function postEvents(
  apiBase: string,
  shortCode: string,
  extensionInstanceId: string,
  rollNumber: string,
  events: TelemetryEvent[],
  telemetryToken: string,
): Promise<boolean> {
  const res = await fetch(`${apiBase}/s/${shortCode}/extension/events`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...(telemetryToken ? { authorization: `Bearer ${telemetryToken}` } : {}),
    },
    body: JSON.stringify({ extensionInstanceId, rollNumber, events }),
  });
  if (res.status === 403) return false;
  if (!res.ok) throw new Error(`Failed to post telemetry batch (${res.status})`);
  return true;
}

export interface RefreshTelemetryTokenResponse {
  telemetryToken: string;
  telemetryTokenExpiresAt: string;
}

/** Reissues a telemetry token for a still-active lock without a full
 * WebAuthn re-pair — see
 * backend-rust/src/controllers/extension_pairing.rs::refresh_telemetry_token.
 * Returns `null` on a 403 (lock no longer active — the caller must fall back
 * to re-pairing), matching `postEvents`'s convention. */
export async function refreshTelemetryToken(
  apiBase: string,
  shortCode: string,
  currentToken: string,
): Promise<RefreshTelemetryTokenResponse | null> {
  const res = await fetch(`${apiBase}/s/${shortCode}/extension/telemetry/token/refresh`, {
    method: 'POST',
    headers: { authorization: `Bearer ${currentToken}` },
  });
  if (res.status === 403 || res.status === 401) return null;
  if (!res.ok) throw new Error(`Failed to refresh telemetry token (${res.status})`);
  return res.json();
}
