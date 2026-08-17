import * as LocalAuthentication from 'expo-local-authentication';
import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { AppState, AppStateStatus } from 'react-native';

import {
  disableBiometricGate,
  enableBiometricGate,
  hasStoredSession,
  isBiometricEnabled,
  unlockWithBiometricGate,
} from '../auth/secureSession';
import { BiometricCapability, getBiometricCapability } from '../utils/biometric';

// How long the app can sit backgrounded before a resume re-locks it. A short
// grace period avoids re-locking on a quick notification-shade pull, while
// still matching the RailOne/PhonePe-style "re-lock on resume" behavior.
const RELOCK_GRACE_MS = 10_000;

interface BiometricLockContextValue {
  bootstrapped: boolean;
  enabled: boolean;
  locked: boolean;
  capability: BiometricCapability | null;
  refreshCapability: () => Promise<BiometricCapability>;
  enroll: () => Promise<{ success: boolean; message?: string }>;
  disable: () => Promise<void>;
  unlock: () => Promise<boolean>;
  unlockWithoutBiometric: () => void;
  resetLock: () => void;
}

const BiometricLockContext = createContext<BiometricLockContextValue | null>(null);

export const useBiometricLock = (): BiometricLockContextValue => {
  const ctx = useContext(BiometricLockContext);
  if (!ctx) throw new Error('useBiometricLock must be used within BiometricLockProvider');
  return ctx;
};

export const BiometricLockProvider = ({ children }: { children: React.ReactNode }) => {
  const [bootstrapped, setBootstrapped] = useState(false);
  const [enabled, setEnabled] = useState(false);
  const [locked, setLocked] = useState(false);
  const [capability, setCapability] = useState<BiometricCapability | null>(null);
  const backgroundedAt = useRef<number | null>(null);

  useEffect(() => {
    (async () => {
      const [storedEnabled, hasSession, caps] = await Promise.all([
        isBiometricEnabled(),
        hasStoredSession(),
        getBiometricCapability(),
      ]);
      setEnabled(storedEnabled);
      // Cold start always locks when biometrics are on, matching banking-app UX.
      setLocked(storedEnabled && hasSession);
      setCapability(caps);
      setBootstrapped(true);
    })();
  }, []);

  useEffect(() => {
    const handleChange = (next: AppStateStatus) => {
      if (next === 'background' || next === 'inactive') {
        backgroundedAt.current = Date.now();
      } else if (next === 'active') {
        const at = backgroundedAt.current;
        backgroundedAt.current = null;
        if (enabled && at !== null && Date.now() - at > RELOCK_GRACE_MS) {
          setLocked(true);
        }
      }
    };
    const sub = AppState.addEventListener('change', handleChange);
    return () => sub.remove();
  }, [enabled]);

  const refreshCapability = useCallback(async () => {
    const caps = await getBiometricCapability();
    setCapability(caps);
    return caps;
  }, []);

  const enroll = useCallback(async (): Promise<{ success: boolean; message?: string }> => {
    const caps = await refreshCapability();
    if (!caps.hasHardware || !caps.isEnrolled) {
      return {
        success: false,
        message: `No ${caps.label.toLowerCase()} set up on this device yet. Enroll it in your phone's Settings first.`,
      };
    }
    const result = await LocalAuthentication.authenticateAsync({
      promptMessage: 'Confirm your identity to enable quick unlock',
    });
    if (!result.success) {
      return { success: false };
    }
    try {
      await enableBiometricGate();
      setEnabled(true);
      setLocked(false);
      return { success: true };
    } catch {
      return {
        success: false,
        message: 'Your device security does not meet the requirements for biometric unlock (e.g., weak face unlock on some Android devices).',
      };
    }
  }, [refreshCapability]);

  const disable = useCallback(async () => {
    await disableBiometricGate();
    setEnabled(false);
    setLocked(false);
  }, []);

  const unlock = useCallback(async (): Promise<boolean> => {
    const success = await unlockWithBiometricGate();
    if (success) setLocked(false);
    return success;
  }, []);

  // "Use password instead" on the unlock screen — falls through to a full
  // re-login rather than fighting a broken/forgotten biometric sensor.
  const unlockWithoutBiometric = useCallback(() => setLocked(false), []);

  const resetLock = useCallback(() => {
    setEnabled(false);
    setLocked(false);
  }, []);

  const value = useMemo(
    () => ({
      bootstrapped,
      enabled,
      locked,
      capability,
      refreshCapability,
      enroll,
      disable,
      unlock,
      unlockWithoutBiometric,
      resetLock,
    }),
    [bootstrapped, enabled, locked, capability, refreshCapability, enroll, disable, unlock, unlockWithoutBiometric, resetLock]
  );

  return <BiometricLockContext.Provider value={value}>{children}</BiometricLockContext.Provider>;
};
