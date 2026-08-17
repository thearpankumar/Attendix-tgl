import * as SecureStore from 'expo-secure-store';
import { Platform } from 'react-native';

import { Admin } from '../api/types';

// Two-tier SecureStore design:
//   - JWT + cached profile live under a *plain* (non-biometric) key so the
//     axios interceptor can read them silently on every request.
//   - A separate sentinel key is written with `requireAuthentication: true`
//     purely so that *reading* it triggers the native Face ID / fingerprint /
//     passcode prompt. Its value is never used for anything — the read
//     itself is the "unlock" action. This keeps per-request auth headers
//     silent while still gating app access behind biometrics.
const KEY_JWT = 'mentor_jwt';
const KEY_PROFILE = 'mentor_profile';
const KEY_BIOMETRIC_ENABLED = 'mentor_biometric_enabled';
const KEY_BIOMETRIC_GATE = 'mentor_biometric_gate';

const plainOptions: SecureStore.SecureStoreOptions = {
  keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
};

// expo-secure-store has no web implementation at all (its native module is a
// bare `{}` on web, so calling it throws) — the app never ships as a web
// build, but `expo start --web` is still the quickest way to eyeball the UI
// on a PC without an emulator, so the plain (non-biometric) keys fall back to
// localStorage there. This is not a security boundary claim for web, just
// enough persistence for a local dev preview.
const plainStore = {
  async setItemAsync(key: string, value: string): Promise<void> {
    if (Platform.OS === 'web') {
      localStorage.setItem(key, value);
      return;
    }
    await SecureStore.setItemAsync(key, value, plainOptions);
  },
  async getItemAsync(key: string): Promise<string | null> {
    if (Platform.OS === 'web') {
      return localStorage.getItem(key);
    }
    return SecureStore.getItemAsync(key, plainOptions);
  },
  async deleteItemAsync(key: string): Promise<void> {
    if (Platform.OS === 'web') {
      localStorage.removeItem(key);
      return;
    }
    await SecureStore.deleteItemAsync(key);
  },
};

export async function saveSession(token: string, admin: Admin): Promise<void> {
  await plainStore.setItemAsync(KEY_JWT, token);
  await plainStore.setItemAsync(KEY_PROFILE, JSON.stringify(admin));
}

export async function getToken(): Promise<string | null> {
  return plainStore.getItemAsync(KEY_JWT);
}

export async function getCachedAdmin(): Promise<Admin | null> {
  const raw = await plainStore.getItemAsync(KEY_PROFILE);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as Admin;
  } catch {
    return null;
  }
}

export async function hasStoredSession(): Promise<boolean> {
  const token = await getToken();
  return !!token;
}

export async function isBiometricEnabled(): Promise<boolean> {
  const flag = await plainStore.getItemAsync(KEY_BIOMETRIC_ENABLED);
  return flag === '1';
}

// Biometric hardware is never reported available on web (see
// BiometricLockContext/getBiometricCapability, backed by
// expo-local-authentication's web shim, which always returns
// hasHardware: false) — so the enrollment UI never calls this on web in
// practice. Guarded here too so it fails safe rather than throwing if that
// ever changes.
export async function enableBiometricGate(): Promise<void> {
  if (Platform.OS === 'web') return;
  await SecureStore.setItemAsync(KEY_BIOMETRIC_GATE, '1', {
    requireAuthentication: true,
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
    authenticationPrompt: 'Unlock Attendix',
  });
  await plainStore.setItemAsync(KEY_BIOMETRIC_ENABLED, '1');
}

export async function disableBiometricGate(): Promise<void> {
  if (Platform.OS !== 'web') {
    await SecureStore.deleteItemAsync(KEY_BIOMETRIC_GATE).catch(() => {});
  }
  await plainStore.setItemAsync(KEY_BIOMETRIC_ENABLED, '0');
}

// Reading the gated sentinel is what triggers the native biometric/passcode
// sheet. Resolves true on success, false on cancel/failure — never throws.
//
// Edge case — Android key invalidation: If the user enrolls a new fingerprint
// after biometric unlock is set up, Android permanently invalidates the
// cryptographic key bound to the old biometric set. getItemAsync then returns
// null (even after a successful prompt), which would permanently lock the user
// out. We detect the null-after-auth case, clean up the stale gate flag, and
// return false so the caller falls through to the "Use password" path where
// the user can log in normally and re-enroll biometrics.
export async function unlockWithBiometricGate(): Promise<boolean> {
  try {
    const value = await SecureStore.getItemAsync(KEY_BIOMETRIC_GATE, {
      requireAuthentication: true,
      authenticationPrompt: 'Unlock Attendix',
    });
    if (value === '1') return true;
    // value is null — key was permanently invalidated (new biometric enrolled).
    // Clean up so the user is not permanently locked out of the app.
    await disableBiometricGate();
    return false;
  } catch {
    return false;
  }
}

export async function clearSession(): Promise<void> {
  await Promise.all([
    plainStore.deleteItemAsync(KEY_JWT).catch(() => {}),
    plainStore.deleteItemAsync(KEY_PROFILE).catch(() => {}),
    plainStore.deleteItemAsync(KEY_BIOMETRIC_ENABLED).catch(() => {}),
    Platform.OS === 'web' ? Promise.resolve() : SecureStore.deleteItemAsync(KEY_BIOMETRIC_GATE).catch(() => {}),
  ]);
}
