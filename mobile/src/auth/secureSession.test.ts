/* eslint-disable import/first -- jest.mock must precede the imports it mocks */
jest.mock('expo-secure-store', () => ({
  WHEN_UNLOCKED_THIS_DEVICE_ONLY: 'WHEN_UNLOCKED_THIS_DEVICE_ONLY',
  setItemAsync: jest.fn().mockResolvedValue(undefined),
  getItemAsync: jest.fn().mockResolvedValue(null),
  deleteItemAsync: jest.fn().mockResolvedValue(undefined),
}));

import * as SecureStore from 'expo-secure-store';

import { Admin } from '../api/types';
import {
  clearSession,
  disableBiometricGate,
  enableBiometricGate,
  getCachedAdmin,
  getToken,
  hasStoredSession,
  isBiometricEnabled,
  saveSession,
  unlockWithBiometricGate,
} from './secureSession';

const mockedSetItem = SecureStore.setItemAsync as jest.Mock;
const mockedGetItem = SecureStore.getItemAsync as jest.Mock;
const mockedDeleteItem = SecureStore.deleteItemAsync as jest.Mock;

const admin: Admin = { _id: '1', username: 'mentor1', email: 'm@example.com', role: 'admin' };

beforeEach(() => {
  jest.clearAllMocks();
});

describe('saveSession / getToken / getCachedAdmin', () => {
  it('stores the token and profile under non-biometric-gated keys', async () => {
    await saveSession('jwt-abc', admin);

    expect(mockedSetItem).toHaveBeenCalledWith(
      'mentor_jwt',
      'jwt-abc',
      expect.objectContaining({ keychainAccessible: 'WHEN_UNLOCKED_THIS_DEVICE_ONLY' })
    );
    expect(mockedSetItem).toHaveBeenCalledWith(
      'mentor_profile',
      JSON.stringify(admin),
      expect.objectContaining({ keychainAccessible: 'WHEN_UNLOCKED_THIS_DEVICE_ONLY' })
    );
    // Neither call should require biometric auth to read back — that would
    // break silent per-request header injection in the axios interceptor.
    expect(mockedSetItem.mock.calls.every(([, , opts]) => !opts?.requireAuthentication)).toBe(true);
  });

  it('getToken reads back the plain jwt key', async () => {
    mockedGetItem.mockResolvedValueOnce('jwt-abc');
    await expect(getToken()).resolves.toBe('jwt-abc');
    expect(mockedGetItem).toHaveBeenCalledWith('mentor_jwt', expect.anything());
  });

  it('getCachedAdmin parses the stored JSON profile', async () => {
    mockedGetItem.mockResolvedValueOnce(JSON.stringify(admin));
    await expect(getCachedAdmin()).resolves.toEqual(admin);
  });

  it('getCachedAdmin returns null instead of throwing on corrupt JSON', async () => {
    mockedGetItem.mockResolvedValueOnce('{not-json');
    await expect(getCachedAdmin()).resolves.toBeNull();
  });

  it('getCachedAdmin returns null when nothing is stored', async () => {
    mockedGetItem.mockResolvedValueOnce(null);
    await expect(getCachedAdmin()).resolves.toBeNull();
  });
});

describe('hasStoredSession', () => {
  it('is true when a token is present', async () => {
    mockedGetItem.mockResolvedValueOnce('jwt-abc');
    await expect(hasStoredSession()).resolves.toBe(true);
  });

  it('is false when no token is present', async () => {
    mockedGetItem.mockResolvedValueOnce(null);
    await expect(hasStoredSession()).resolves.toBe(false);
  });
});

describe('biometric gate', () => {
  it('enableBiometricGate writes a requireAuthentication-gated sentinel and sets the plain enabled flag', async () => {
    await enableBiometricGate();

    expect(mockedSetItem).toHaveBeenCalledWith(
      'mentor_biometric_gate',
      '1',
      expect.objectContaining({ requireAuthentication: true })
    );
    expect(mockedSetItem).toHaveBeenCalledWith('mentor_biometric_enabled', '1', expect.anything());
  });

  it('disableBiometricGate deletes the gate key and flips the flag to "0"', async () => {
    await disableBiometricGate();

    expect(mockedDeleteItem).toHaveBeenCalledWith('mentor_biometric_gate');
    expect(mockedSetItem).toHaveBeenCalledWith('mentor_biometric_enabled', '0', expect.anything());
  });

  it('isBiometricEnabled is true only when the flag is exactly "1"', async () => {
    mockedGetItem.mockResolvedValueOnce('1');
    await expect(isBiometricEnabled()).resolves.toBe(true);

    mockedGetItem.mockResolvedValueOnce('0');
    await expect(isBiometricEnabled()).resolves.toBe(false);

    mockedGetItem.mockResolvedValueOnce(null);
    await expect(isBiometricEnabled()).resolves.toBe(false);
  });

  it('unlockWithBiometricGate resolves true when the OS prompt succeeds and returns the sentinel', async () => {
    mockedGetItem.mockResolvedValueOnce('1');
    await expect(unlockWithBiometricGate()).resolves.toBe(true);
    expect(mockedGetItem).toHaveBeenCalledWith('mentor_biometric_gate', expect.objectContaining({ requireAuthentication: true }));
  });

  it('unlockWithBiometricGate resolves false (never throws) when the prompt is cancelled', async () => {
    mockedGetItem.mockRejectedValueOnce(new Error('user cancelled'));
    await expect(unlockWithBiometricGate()).resolves.toBe(false);
  });

  it('unlockWithBiometricGate cleans up and resolves false when Android has invalidated the key (successful prompt, null value)', async () => {
    // A successful prompt but a null value back means the OS accepted the
    // (possibly new) biometric but the key bound to the old enrollment no
    // longer decrypts — Android does this permanently after re-enrollment.
    mockedGetItem.mockResolvedValueOnce(null);
    await expect(unlockWithBiometricGate()).resolves.toBe(false);
    // Falls through to disableBiometricGate(), so the stale gate + enabled
    // flag don't keep re-prompting a sensor that can never succeed again.
    expect(mockedDeleteItem).toHaveBeenCalledWith('mentor_biometric_gate');
    expect(mockedSetItem).toHaveBeenCalledWith('mentor_biometric_enabled', '0', expect.anything());
  });
});

describe('clearSession', () => {
  it('deletes all four session-related keys', async () => {
    await clearSession();

    const deletedKeys = mockedDeleteItem.mock.calls.map(([key]) => key).sort();
    expect(deletedKeys).toEqual(['mentor_biometric_enabled', 'mentor_biometric_gate', 'mentor_jwt', 'mentor_profile'].sort());
  });

  it('does not throw even if an individual delete rejects', async () => {
    mockedDeleteItem.mockRejectedValueOnce(new Error('key not found'));
    await expect(clearSession()).resolves.toBeUndefined();
  });
});
