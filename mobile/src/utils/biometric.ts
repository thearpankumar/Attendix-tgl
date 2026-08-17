import * as LocalAuthentication from 'expo-local-authentication';
import { Platform } from 'react-native';

export interface BiometricCapability {
  hasHardware: boolean;
  isEnrolled: boolean;
  types: LocalAuthentication.AuthenticationType[];
  // Human-readable label for the enrolled method, e.g. "Face ID" on iOS or
  // "Fingerprint" on Android — used in enrollment/unlock/settings copy.
  label: string;
}

export async function getBiometricCapability(): Promise<BiometricCapability> {
  const [hasHardware, isEnrolled, types] = await Promise.all([
    LocalAuthentication.hasHardwareAsync(),
    LocalAuthentication.isEnrolledAsync(),
    LocalAuthentication.supportedAuthenticationTypesAsync(),
  ]);

  return { hasHardware, isEnrolled, types, label: describeBiometricType(types) };
}

export function describeBiometricType(types: LocalAuthentication.AuthenticationType[]): string {
  const hasFace = types.includes(LocalAuthentication.AuthenticationType.FACIAL_RECOGNITION);
  const hasFingerprint = types.includes(LocalAuthentication.AuthenticationType.FINGERPRINT);
  const hasIris = types.includes(LocalAuthentication.AuthenticationType.IRIS);

  if (Platform.OS === 'android') {
    // Many Android devices report FACIAL_RECOGNITION hardware capability
    // even when the user has only enrolled a fingerprint (front-camera face
    // unlock exists but isn't set up as the secure method) — prefer
    // fingerprint, and never say "Face ID" here, since that's Apple's own
    // product name, not a generic term.
    if (hasFingerprint) return 'Fingerprint';
    if (hasFace) return 'Face unlock';
    if (hasIris) return 'Iris scan';
    return 'Biometric unlock';
  }

  // iOS devices have exactly one or the other, never both.
  if (hasFace) return 'Face ID';
  if (hasFingerprint) return 'Touch ID';
  return 'Biometric unlock';
}
