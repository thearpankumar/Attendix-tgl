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
});

// No cookie jar needed: the app authenticates with a plain Authorization
// header only, and the backend's CSRF middleware exempts cookie-less
// Bearer-authenticated requests (see backend-rust/src/middleware/csrf.rs).
//
// The catch: `POST /admin/login` (shared with the web frontends) always
// sets an HttpOnly `admin_token` cookie in its response, and on Android,
// React Native's networking layer sits on OkHttp, which — unlike a typical
// server-side HTTP client — has a *persistent* CookieJar that silently
// stores any Set-Cookie it sees and replays it on later requests to the
// same origin, exactly like a browser would. So after logging in, this
// app's own writes end up carrying that admin_token cookie alongside the
// explicit Bearer header, which defeats the cookie-less exemption above and
// 403s with "CSRF token missing" (this app never does the CSRF
// cookie/header dance). Clearing cookies on module load (in case a stray
// cookie survived from before this fix) and after every response keeps the
// app's actual behavior matching its "no cookies at all" design.
//
// `require`d lazily, only on native platforms: the package's own module
// throws at *import* time (before any of our own guards run) on any
// platform other than ios/android — which crashes `expo export --platform
// web` (used by web preview and the CI build check) if this were a static
// top-level import.
function clearStrayCookies() {
  if (Platform.OS === 'ios' || Platform.OS === 'android') {
    // eslint-disable-next-line @typescript-eslint/no-require-imports -- must stay a runtime require, see comment above.
    const CookieManager = require('@react-native-cookies/cookies').default;
    CookieManager.clearAll().catch(() => {});
  }
}
clearStrayCookies();

api.interceptors.request.use(async (config) => {
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
