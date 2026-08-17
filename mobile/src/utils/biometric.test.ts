import { AuthenticationType } from 'expo-local-authentication';
import { Platform } from 'react-native';

import { describeBiometricType } from './biometric';

describe('describeBiometricType on iOS (default test platform)', () => {
  it('prefers Face ID when facial recognition is supported', () => {
    expect(describeBiometricType([AuthenticationType.FACIAL_RECOGNITION, AuthenticationType.FINGERPRINT])).toBe(
      'Face ID'
    );
  });

  it('labels fingerprint-only hardware as Touch ID', () => {
    expect(describeBiometricType([AuthenticationType.FINGERPRINT])).toBe('Touch ID');
  });

  it('falls back to a generic label when nothing is supported', () => {
    expect(describeBiometricType([])).toBe('Biometric unlock');
  });
});

describe('describeBiometricType on Android', () => {
  const originalOS = Platform.OS;

  beforeEach(() => {
    Object.defineProperty(Platform, 'OS', { get: () => 'android' });
  });

  afterEach(() => {
    Object.defineProperty(Platform, 'OS', { get: () => originalOS });
  });

  it('prefers Fingerprint even when facial recognition hardware is also reported', () => {
    expect(describeBiometricType([AuthenticationType.FACIAL_RECOGNITION, AuthenticationType.FINGERPRINT])).toBe(
      'Fingerprint'
    );
  });

  it('labels face-only hardware as "Face unlock", never "Face ID"', () => {
    expect(describeBiometricType([AuthenticationType.FACIAL_RECOGNITION])).toBe('Face unlock');
  });

  it('labels iris-only hardware as Iris scan', () => {
    expect(describeBiometricType([AuthenticationType.IRIS])).toBe('Iris scan');
  });

  it('falls back to a generic label when nothing is supported', () => {
    expect(describeBiometricType([])).toBe('Biometric unlock');
  });
});
