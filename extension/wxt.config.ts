import { defineConfig } from 'wxt';

// See https://wxt.dev/api/config.html
export default defineConfig({
  modules: ['@wxt-dev/module-react'],
  manifest: {
    name: 'Attendix Session Monitor',
    description: 'Pairs this device to a monitored Attendix session and reports presence/activity telemetry for its duration.',
    version: '0.1.0',
    // `windows` deliberately absent: chrome.windows/browser.windows
    // (onFocusChanged/onCreated/onRemoved/getAll, used in background.ts for
    // window-focus/window-count tracking) is an unprivileged API in both
    // Chrome and Firefox — no permission needed. It was never a valid
    // permission string to begin with; Chrome silently ignores an unknown
    // entry, but Firefox's manifest schema validation rejects it outright.
    permissions: [
      'storage',
      'alarms',
      'idle',
      'tabs',
      'tabGroups',
      'history',
    ],
    // Backend origins the extension talks to — same-origin as the site the
    // pairing link opens on (PUBLIC_BASE_URL), plus localhost for dev.
    host_permissions: ['http://localhost/*', 'https://*/*'],
    browser_specific_settings: {
      gecko: {
        // Stable id for temporary `about:debugging` loads and any future
        // signed install — Firefox recommends this for MV2, requires it for
        // MV3. Not a real Mozilla-registered account id, just a fixed
        // identifier so this extension's storage/permissions persist across
        // reloads instead of getting a fresh random id each time.
        id: 'attendix-session-monitor@talenciaglobal.com',
      },
    },
  },
  // `firefoxDataCollection`: Firefox requires new AMO submissions (from
  // 2025-11-03) to declare `data_collection_permissions` describing what
  // personal data categories are collected — this extension genuinely
  // collects several (browsing history, device fingerprint), so that
  // declaration needs a real compliance decision before ever publishing to
  // AMO, not a guessed value here. Suppressed for now since this extension
  // is only sideloaded/temporarily loaded, never submitted — revisit before
  // any AMO submission.
  suppressWarnings: {
    firefoxDataCollection: true,
  },
});
