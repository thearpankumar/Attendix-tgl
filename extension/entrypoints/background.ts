import { browser } from 'wxt/browser';
import { getOrCreateExtensionInstanceId, getPairingState, clearPairingState, enqueueEvent, drainEventQueue, requeueEvents } from '../lib/storage';
import { postEvents } from '../lib/api';
import type { TelemetryEvent, TelemetryEventType, PairingState } from '../lib/types';

const FLUSH_ALARM = 'attendix-telemetry-flush';
// 20s: within the plan's 15-30s batching window, short enough that a
// device-lock loss (403) is noticed and surfaced to the popup promptly.
const FLUSH_INTERVAL_MINUTES = 20 / 60;
// Must match MAX_BATCH_SIZE in backend-rust/src/controllers/telemetry.rs.
// Without this, a backlog built up during a connectivity outage (the local
// queue tolerates up to 2000 events — see lib/storage.ts) would get REJECTED
// by the backend's own 200-event cap on every retry, forever, since the
// queue never shrinks below 200 — draining and resending the whole thing in
// one POST just reproduces the same 400 every tick.
const MAX_BATCH_SIZE = 200;
const IDLE_DETECTION_SECONDS = 60;
const DEFAULT_TARGET_TAB_PATTERN = 'meet.google.com';

function record(eventType: TelemetryEventType, eventData: Record<string, unknown> = {}): void {
  const event: TelemetryEvent = { eventType, eventData, recordedAt: new Date().toISOString() };
  void enqueueEvent(event);
}

async function withActivePairing(fn: (state: PairingState) => void | Promise<void>): Promise<void> {
  const state = await getPairingState();
  if (!state) return;
  await fn(state);
}

async function targetTabInfo(): Promise<{ meetTabOpen: boolean; targetTabFocused: boolean }> {
  const state = await getPairingState();
  const pattern = state?.targetTabUrlPattern ?? DEFAULT_TARGET_TAB_PATTERN;
  const tabs = await browser.tabs.query({});
  const meetTabOpen = tabs.some((t) => t.url?.includes(pattern));
  const [activeTab] = await browser.tabs.query({ active: true, lastFocusedWindow: true });
  const targetTabFocused = Boolean(activeTab?.url?.includes(pattern));
  return { meetTabOpen, targetTabFocused };
}

async function recordTargetTabSignals(): Promise<void> {
  const { meetTabOpen, targetTabFocused } = await targetTabInfo();
  record('meet_tab_open', { open: meetTabOpen });
  record('target_tab_focus', { focused: targetTabFocused });
}

export default defineBackground(() => {
  // Registered synchronously at top level so the browser re-attaches these
  // listeners every time it wakes the service worker for a matching event —
  // an MV3 requirement, not just a style preference.

  browser.runtime.onInstalled.addListener(async (details) => {
    await getOrCreateExtensionInstanceId();
    record('extension_lifecycle', { event: details.reason }); // 'install' | 'update'
  });

  browser.runtime.onStartup.addListener(() => {
    record('extension_lifecycle', { event: 'browser_startup' });
  });

  browser.idle.setDetectionInterval(IDLE_DETECTION_SECONDS);
  browser.idle.onStateChanged.addListener((state) => {
    // 'active' | 'idle' | 'locked'
    record('idle_state', { state });
  });

  browser.tabs.onActivated.addListener(async () => {
    void withActivePairing(async () => {
      await recordTargetTabSignals();
    });
  });

  browser.tabs.onCreated.addListener(() => {
    void withActivePairing(async () => {
      const tabs = await browser.tabs.query({});
      record('tab_count', { count: tabs.length, change: 'opened' });
    });
  });

  browser.tabs.onRemoved.addListener(() => {
    void withActivePairing(async () => {
      const tabs = await browser.tabs.query({});
      record('tab_count', { count: tabs.length, change: 'closed' });
    });
  });

  browser.tabs.onUpdated.addListener((_tabId, changeInfo, tab) => {
    const url = changeInfo.url;
    if (!url) return;
    void withActivePairing(async () => {
      // Domain-level, not the full URL with query/hash — enough signal for
      // "what site were they on" without over-collecting path/query content.
      let domain = url;
      try {
        domain = new URL(url).hostname;
      } catch {
        /* not a real URL (e.g. chrome://), keep the raw string */
      }
      record('tab_url_change', { domain, active: tab.active ?? false });
      await recordTargetTabSignals();
    });
  });

  // chrome.tabGroups is Chromium-only (MV3) — guard for cross-browser builds
  // (Firefox/Safari via WXT don't implement it).
  const tabGroups = (browser as typeof browser & { tabGroups?: typeof chrome.tabGroups }).tabGroups;
  if (tabGroups) {
    const recordGroupChange = (reason: string) => (group: chrome.tabGroups.TabGroup) => {
      void withActivePairing(() => {
        record('tab_group', { reason, title: group.title ?? null, color: group.color });
      });
    };
    tabGroups.onCreated.addListener(recordGroupChange('created'));
    tabGroups.onUpdated.addListener(recordGroupChange('updated'));
  }

  browser.windows.onFocusChanged.addListener((windowId) => {
    void withActivePairing(() => {
      record('window_focus', { focused: windowId !== browser.windows.WINDOW_ID_NONE });
    });
  });

  browser.windows.onCreated.addListener(() => {
    void withActivePairing(async () => {
      const windows = await browser.windows.getAll();
      record('window_count', { count: windows.length, change: 'opened' });
    });
  });

  browser.windows.onRemoved.addListener(() => {
    void withActivePairing(async () => {
      const windows = await browser.windows.getAll();
      record('window_count', { count: windows.length, change: 'closed' });
    });
  });

  // `history` permission covers navigation across ALL sites while the
  // extension is active, not just the target tab — this is the item the
  // plan flags as `(legal issue)`, which is why the popup shows an explicit
  // consent screen before pairing (and therefore before monitoring, since
  // events are only enqueued `withActivePairing`) is ever possible.
  browser.history.onVisited.addListener((result) => {
    void withActivePairing(() => {
      let domain = result.url ?? '';
      try {
        domain = result.url ? new URL(result.url).hostname : '';
      } catch {
        /* keep raw */
      }
      record('navigation_history', { domain, title: result.title ?? null });
    });
  });

  // Idle-detection fallback for browsers/contexts where chrome.idle's
  // interval granularity (minimum ~15s) is too coarse — a raw input event
  // observed anywhere counts as "still active" without recording *what* was
  // typed/clicked (content is never read, only that activity occurred).
  interface PairingCompleteMessage {
    type: 'PAIRING_COMPLETE';
    deviceFingerprint?: Record<string, unknown>;
    uaData?: Record<string, unknown> | null;
    multiMonitor?: boolean | null;
    automationFlags?: string[];
    iceCandidateAddresses?: string[];
  }
  interface InputActivityMessage {
    type: 'INPUT_ACTIVITY';
  }
  interface TabVisibilityMessage {
    type: 'TAB_VISIBILITY';
    visible: boolean;
  }
  interface RequestStateMessage {
    type: 'REQUEST_STATE';
  }
  type BackgroundMessage =
    | PairingCompleteMessage
    | InputActivityMessage
    | TabVisibilityMessage
    | RequestStateMessage;

  browser.runtime.onMessage.addListener((message: BackgroundMessage, _sender, sendResponse) => {
    if (message?.type === 'INPUT_ACTIVITY') {
      void withActivePairing(() => record('input_activity', {}));
      return false;
    }
    if (message?.type === 'TAB_VISIBILITY') {
      void withActivePairing(() => record('tab_visibility', { visible: message.visible }));
      return false;
    }
    if (message?.type === 'PAIRING_COMPLETE') {
      // The popup already wrote the new PairingState to storage before
      // sending this — just make sure the flush alarm is running, record the
      // device signals it gathered (a real `document` context, which the
      // service worker itself doesn't have), and do an immediate heartbeat
      // so the dashboard sees data right away.
      void ensureFlushAlarm();
      void withActivePairing(async (state) => {
        // Uninstall tracking (Part A item 1): re-registered on every
        // successful pairing since this is the only point the background
        // script has both a live `apiBase` and a confirmed instance id to
        // build the URL from — there's no `management`-permission API for
        // detecting an in-place disable, so that half of item 1 is covered
        // by heartbeat-gap detection server-side instead.
        browser.runtime.setUninstallURL(
          `${state.apiBase}/s/extension/uninstall?extensionInstanceId=${state.extensionInstanceId}`,
        );
        if (message.deviceFingerprint) record('device_fingerprint', message.deviceFingerprint);
        if (message.uaData) record('ua_high_entropy', message.uaData);
        if (typeof message.multiMonitor === 'boolean') {
          record('multi_monitor', { detected: message.multiMonitor });
        }
        if (message.automationFlags && message.automationFlags.length > 0) {
          record('emulator_flag', { flags: message.automationFlags });
        }
        if (message.iceCandidateAddresses) {
          record('ip_webrtc_check', { addresses: message.iceCandidateAddresses });
        }
        record('heartbeat', {});
        await recordTargetTabSignals();
      });
      return false;
    }
    if (message?.type === 'REQUEST_STATE') {
      void getPairingState().then((state) => sendResponse({ pairingState: state }));
      return true; // keep the channel open for the async sendResponse
    }
    return false;
  });

  async function ensureFlushAlarm(): Promise<void> {
    const existing = await browser.alarms.get(FLUSH_ALARM);
    if (!existing) {
      browser.alarms.create(FLUSH_ALARM, { periodInMinutes: FLUSH_INTERVAL_MINUTES });
    }
  }

  browser.alarms.onAlarm.addListener((alarm) => {
    if (alarm.name !== FLUSH_ALARM) return;
    void flushQueue();
  });

  async function flushQueue(): Promise<void> {
    const state = await getPairingState();
    if (!state) return;

    record('heartbeat', {});
    const events = await drainEventQueue();
    if (events.length === 0) return;

    // Send in <=MAX_BATCH_SIZE chunks, sequentially and in order. A failed
    // chunk requeues itself plus every chunk after it (unsent chunks stay
    // unsent, in their original order) and stops this tick — the next alarm
    // tick picks up where this one left off.
    for (let i = 0; i < events.length; i += MAX_BATCH_SIZE) {
      const chunk = events.slice(i, i + MAX_BATCH_SIZE);
      try {
        const stillLocked = await postEvents(
          state.apiBase,
          state.shortCode,
          state.extensionInstanceId,
          state.rollNumber,
          chunk,
        );
        if (!stillLocked) {
          // Another device superseded this session's lock (Part E step 8) —
          // stop reporting and let the popup re-show the pairing UI rather
          // than silently retrying forever against a lock we no longer hold.
          await clearPairingState();
          return;
        }
      } catch {
        // Network error, not a lock loss — keep this chunk and everything
        // after it for the next tick instead of dropping them.
        await requeueEvents(events.slice(i));
        return;
      }
    }
  }

  void ensureFlushAlarm();
  void withActivePairing(() => recordTargetTabSignals());
});
