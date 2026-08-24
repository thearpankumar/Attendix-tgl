import { browser } from 'wxt/browser';
import type { PairingState, PendingPairing, TelemetryEvent } from './types';

const KEYS = {
  extensionInstanceId: 'extensionInstanceId',
  pairingState: 'pairingState',
  pendingPairing: 'pendingPairing',
  eventQueue: 'eventQueue',
  consentAcknowledged: 'consentAcknowledged',
} as const;

/** The durable "which physical install is this" anchor — generated once,
 * reused across every session this install ever pairs with. */
export async function getOrCreateExtensionInstanceId(): Promise<string> {
  const stored = await browser.storage.local.get(KEYS.extensionInstanceId);
  const existing = stored[KEYS.extensionInstanceId] as string | undefined;
  if (existing) return existing;
  const id = crypto.randomUUID();
  await browser.storage.local.set({ [KEYS.extensionInstanceId]: id });
  return id;
}

export async function getConsentAcknowledged(): Promise<boolean> {
  const stored = await browser.storage.local.get(KEYS.consentAcknowledged);
  return Boolean(stored[KEYS.consentAcknowledged]);
}

export async function setConsentAcknowledged(): Promise<void> {
  await browser.storage.local.set({ [KEYS.consentAcknowledged]: true });
}

export async function getPairingState(): Promise<PairingState | null> {
  const stored = await browser.storage.local.get(KEYS.pairingState);
  return (stored[KEYS.pairingState] as PairingState | undefined) ?? null;
}

export async function setPairingState(state: PairingState): Promise<void> {
  await browser.storage.local.set({ [KEYS.pairingState]: state });
}

/** Called when the backend rejects a batch (403 — device no longer locked,
 * e.g. a re-pair on another device superseded this one). Drops the local
 * pairing state so the popup goes back to "Pair this device". */
export async function clearPairingState(): Promise<void> {
  await browser.storage.local.remove(KEYS.pairingState);
}

export async function getPendingPairing(): Promise<PendingPairing | null> {
  const stored = await browser.storage.local.get(KEYS.pendingPairing);
  return (stored[KEYS.pendingPairing] as PendingPairing | undefined) ?? null;
}

export async function setPendingPairing(pairing: PendingPairing | null): Promise<void> {
  if (pairing === null) {
    await browser.storage.local.remove(KEYS.pendingPairing);
  } else {
    await browser.storage.local.set({ [KEYS.pendingPairing]: pairing });
  }
}

/** Ceiling on the persisted queue — ~10 flush cycles' worth at the ingest
 * endpoint's MAX_BATCH_SIZE (backend-rust/src/controllers/telemetry.rs).
 * Without a cap, a prolonged backend outage would let `storage.local` grow
 * unbounded; beyond this, the oldest events are dropped in favor of newer
 * ones so the queue always reflects recent activity rather than stalling
 * entirely on quota. */
const MAX_QUEUE_SIZE = 2000;

function capQueue(queue: TelemetryEvent[]): TelemetryEvent[] {
  if (queue.length <= MAX_QUEUE_SIZE) return queue;
  return queue.slice(queue.length - MAX_QUEUE_SIZE);
}

/** Event queue is persisted (not just held in a module-level array) because
 * the MV3 background service worker can be killed and restarted between
 * collector firing and the next alarm-driven flush. */
export async function enqueueEvent(event: TelemetryEvent): Promise<void> {
  const stored = await browser.storage.local.get(KEYS.eventQueue);
  const queue = (stored[KEYS.eventQueue] as TelemetryEvent[] | undefined) ?? [];
  queue.push(event);
  await browser.storage.local.set({ [KEYS.eventQueue]: capQueue(queue) });
}

export async function drainEventQueue(): Promise<TelemetryEvent[]> {
  const stored = await browser.storage.local.get(KEYS.eventQueue);
  const queue = (stored[KEYS.eventQueue] as TelemetryEvent[] | undefined) ?? [];
  await browser.storage.local.remove(KEYS.eventQueue);
  return queue;
}

/** Puts events back at the front of the queue after a failed flush, so a
 * transient network error doesn't lose them. */
export async function requeueEvents(events: TelemetryEvent[]): Promise<void> {
  if (events.length === 0) return;
  const stored = await browser.storage.local.get(KEYS.eventQueue);
  const queue = (stored[KEYS.eventQueue] as TelemetryEvent[] | undefined) ?? [];
  await browser.storage.local.set({ [KEYS.eventQueue]: capQueue([...events, ...queue]) });
}
