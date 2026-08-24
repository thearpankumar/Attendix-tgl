import { API_BASE_URL } from './config';

// Matches `/s/{code}` OR `/attend/{code}` — NOT `/attend/pair/...` or
// `/attend/legacy/...`, which are different pages entirely (the phone-side
// pairing ceremony and the legacy attendance flow, respectively; see
// `frontend/student/src/App.tsx`'s routes). `/s/{code}` is the link the
// admin actually shares (`Sessions.tsx`), but Caddy proxies every `/s/*`
// request straight to the backend, and `resolve_short_link` (`GET
// /s/{shortCode}`) always 302s to `/attend/{shortCode}` — so by the time
// the popup reads the active tab's URL, a real browser has *already*
// followed that redirect and settled on `/attend/{code}`, never `/s/{code}`.
// The `/s/` alternative is kept only as a defensive fallback (e.g. a dev
// setup without Caddy's rewrite in front of it).
export const SHORT_CODE_PATTERN = /\/(?:s|attend(?!\/(?:pair|legacy)\/))\/([a-z0-9-]+)/i;

/**
 * Only extracts the short code from the active tab's URL — `apiBase` is
 * NOT derived from it. This used to build `apiBase` from the tab's own
 * origin (`${protocol}//${host}/api`), which meant the extension would
 * talk to whatever backend the open page happened to be served from. It
 * now always talks to the one backend configured at build time — see
 * `config.ts`.
 */
export function detectShortCodeFromUrl(url: string | undefined): { apiBase: string; shortCode: string } | null {
  if (!url) return null;
  const match = SHORT_CODE_PATTERN.exec(url);
  const shortCode = match?.[1];
  if (!shortCode) return null;
  return { apiBase: API_BASE_URL, shortCode };
}
