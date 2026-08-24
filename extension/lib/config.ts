/**
 * Backend/site origin this extension talks to — always read from a
 * build-time env var (this folder's own `.env`, `WXT_API_BASE_URL`), never
 * derived from whatever page happens to be open in the active tab. That
 * used to be how it worked (see git history of `App.tsx`'s
 * `detectShortCodeFromUrl`), which meant the extension would happily talk
 * to *any* origin a session link was opened from — fine for same-origin
 * dev/staging/prod setups, but not something to rely on for a piece of
 * software that reports device fingerprints and browsing history back to
 * "wherever this tab happens to be pointed."
 *
 * Falls back to `http://localhost` only so `wxt dev`/`wxt build` still work
 * without a `.env` file present (matches this repo's local dev origin — see
 * root `.env.example`'s `PUBLIC_BASE_URL`). A real build should always set
 * `WXT_API_BASE_URL` explicitly in `.env` — see `.env.example` in this
 * folder.
 */
const SITE_ORIGIN = import.meta.env.WXT_API_BASE_URL ?? 'http://localhost';

/** Every backend call this extension makes is `${API_BASE_URL}/s/{shortCode}/...`. */
export const API_BASE_URL = `${SITE_ORIGIN}/api`;
