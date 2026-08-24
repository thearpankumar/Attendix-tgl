import { browser } from 'wxt/browser';

// Idle-detection fallback (Part A "Mouse/keyboard activity ... as an
// idle-detection fallback"): reports only that *an* input event occurred
// somewhere on the page, throttled to at most once every 10s — never what
// was typed or clicked. `chrome.idle` (background.ts) is the primary
// presence signal; this exists because its minimum detection interval
// (~15s) is coarser than some monitoring windows want.
export default defineContentScript({
  matches: ['<all_urls>'],
  main() {
    let lastSent = 0;
    const THROTTLE_MS = 10_000;

    const report = () => {
      const now = Date.now();
      if (now - lastSent < THROTTLE_MS) return;
      lastSent = now;
      void browser.runtime.sendMessage({ type: 'INPUT_ACTIVITY' }).catch(() => {
        // Background service worker not reachable (e.g. extension
        // reloading) — harmless to drop a single presence ping.
      });
    };

    window.addEventListener('mousemove', report, { passive: true });
    window.addEventListener('keydown', report, { passive: true });
    window.addEventListener('touchstart', report, { passive: true });

    // Tab visible/hidden (Part A item 4): fires immediately on switch, no
    // throttling needed since visibilitychange itself is already a discrete,
    // low-frequency event.
    document.addEventListener('visibilitychange', () => {
      void browser.runtime
        .sendMessage({ type: 'TAB_VISIBILITY', visible: document.visibilityState === 'visible' })
        .catch(() => {
          // Background service worker not reachable — harmless to drop one
          // visibility transition.
        });
    });
  },
});
