import axios, { AxiosError } from 'axios';
import Constants from 'expo-constants';
import { Platform } from 'react-native';
import Toast from 'react-native-toast-message';

import { getToken } from '../auth/secureSession';
import { ApiErrorBody } from './types';

const apiBaseUrl =
  (Constants.expoConfig?.extra?.apiBaseUrl as string | undefined) ??
  'https://attendixv2.talenciaglobal.com/api';

if (__DEV__ && !apiBaseUrl.startsWith('https://') && !apiBaseUrl.includes('localhost') && !/^https?:\/\/(\d{1,3}\.){3}\d{1,3}/.test(apiBaseUrl)) {
  console.warn(`[api] apiBaseUrl "${apiBaseUrl}" does not look like HTTPS or a LAN dev address.`);
}

// eslint-disable-next-line import/no-named-as-default-member -- axios's CJS/ESM interop exposes `create` both ways; this is the documented default-import usage.
export const api = axios.create({
  baseURL: apiBaseUrl,
  timeout: 15000,
  // Tells POST /admin/login (shared with the web frontends) to skip setting
  // its HttpOnly admin_token cookie for this client entirely — see
  // backend-rust/src/middleware/auth.rs's SKIP_AUTH_COOKIE_HEADER doc
  // comment for why: relying on this app to reliably scrub that cookie
  // client-side (below) turned out to be racy on Android in practice, so
  // the real fix is having the server never set it here in the first place.
  headers: { 'X-Attendix-No-Cookie': '1' },
});

// Defense-in-depth only now that the server skips Set-Cookie for this
// client (see the header above): an install that already picked up a
// stray admin_token cookie from before that server change shipped would
// otherwise keep failing until this runs once. No cookie jar is needed
// for anything else — the app authenticates with a plain Authorization
// header only, and the backend's CSRF middleware exempts cookie-less
// Bearer-authenticated requests (see backend-rust/src/middleware/csrf.rs).
//
// `require`d lazily, only on native platforms: the package's own module
// throws at *import* time (before any of our own guards run) on any
// platform other than ios/android — which crashes `expo export --platform
// web` (used by web preview and the CI build check) if this were a static
// top-level import.
//
// The `CookieManager?.clearAll` guard below is load-bearing, not defensive
// filler: this native module isn't bundled in plain Expo Go (only in this
// app's own custom dev-client/production builds), so `require(...).default`
// resolves to `undefined` there. Without the guard, calling `.clearAll()`
// throws synchronously at module-eval time, which crashes evaluation of
// every route module that (transitively) imports this file — the actual
// cause behind expo-router's unrelated-looking "missing default export"
// warnings for every single route.
//
// `clearAll()` resolving is NOT enough on Android: it clears the in-memory
// CookieManager, but OkHttp's `ForwardingCookieHandler` reads from the
// underlying persistent store, which `clearAll()` alone doesn't guarantee
// has synced yet — the next request can still race ahead and pick up the
// stale cookie even after `clearAll()`'s promise resolves. `flush()`
// (Android-only) forces that sync before we consider clearing done.
async function clearStrayCookies(): Promise<void> {
  if (Platform.OS === 'ios' || Platform.OS === 'android') {
    // eslint-disable-next-line @typescript-eslint/no-require-imports -- must stay a runtime require, see comment above.
    const CookieManager = require('@react-native-cookies/cookies').default;
    await CookieManager?.clearAll().catch(() => {});
    if (Platform.OS === 'android') {
      await CookieManager?.flush?.().catch(() => {});
    }
  }
}
clearStrayCookies();

// Clearing was previously only done fire-and-forget in the *response*
// interceptor below, which raced the very next request: `clearAll()` is an
// async bridge call to native code, so a request fired immediately after
// login (e.g. the roster fetch, then the first present/absent mark) could
// go out before that promise settled, still carrying the stray admin_token
// cookie — which is exactly what tripped "CSRF token missing" on the first
// POST after login in production. Awaiting it here, before every outgoing
// request, guarantees the cookie jar is empty at send time, every time.
api.interceptors.request.use(async (config) => {
  await clearStrayCookies();
  const token = await getToken();
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Registered by AuthProvider so the interceptor can drop the session on a
// 401 without importing React context into this module.
let unauthorizedHandler: (() => void) | null = null;
export function setUnauthorizedHandler(handler: (() => void) | null) {
  unauthorizedHandler = handler;
}

api.interceptors.response.use(
  (response) => {
    clearStrayCookies();
    return response;
  },
  (error: AxiosError<ApiErrorBody>) => {
    clearStrayCookies();
    const status = error.response?.status;

    if (status === 401) {
      unauthorizedHandler?.();
    } else if (status === 429) {
      Toast.show({
        type: 'error',
        text1: 'Too many requests',
        text2: error.response?.data?.error || 'Please slow down and try again shortly.',
      });
    }

    return Promise.reject(error);
  }
);

export function apiErrorMessage(error: unknown, fallback: string): string {
  const err = error as AxiosError<ApiErrorBody> | undefined;
  return err?.response?.data?.message || err?.response?.data?.error || fallback;
}
