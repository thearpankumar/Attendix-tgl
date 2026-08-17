import { useRouter } from 'expo-router';
import { Fingerprint } from 'lucide-react-native';
import React, { useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { Button } from '../components/ui/Button';
import { useAuth } from '../context/AuthContext';
import { useBiometricLock } from '../context/BiometricLockContext';
import { useTheme } from '../theme/ThemeProvider';

export default function EnrollBiometric() {
  const { colors, radii, font } = useTheme();
  const { clearJustAuthenticated } = useAuth();
  const biometricLock = useBiometricLock();
  const router = useRouter();
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const label = biometricLock.capability?.label ?? 'biometric unlock';

  const finish = () => {
    clearJustAuthenticated();
    router.replace('/');
  };

  const handleEnable = async () => {
    setSubmitting(true);
    setError('');
    try {
      const result = await biometricLock.enroll();
      if (result.success) {
        finish();
      } else if (result.message) {
        setError(result.message);
      }
    } catch {
      setError('An unexpected error occurred. Please try again.');
    } finally {
      setSubmitting(false);
    }
    // A plain cancel (no message) just leaves the prompt on screen to retry or skip.
  };

  return (
    <View style={[styles.screen, { backgroundColor: colors.bg }]}>
      <View style={[styles.card, { backgroundColor: colors.surface, borderColor: colors.border, borderRadius: radii.lg }]}>
        <View style={[styles.iconWrap, { backgroundColor: colors.primaryLight }]}>
          <Fingerprint size={32} color={colors.primary} />
        </View>
        <Text style={[styles.title, { color: colors.text, fontFamily: font.extrabold }]}>Enable {label}?</Text>
        <Text style={[styles.subtitle, { color: colors.muted, fontFamily: font.regular }]}>
          Unlock Attendix instantly next time using {label.toLowerCase()} instead of typing your password.
        </Text>

        {error ? (
          <View style={[styles.errorBanner, { backgroundColor: colors.dangerBg, borderRadius: radii.sm }]}>
            <Text style={{ color: colors.dangerTxt, fontSize: 13 }}>{error}</Text>
          </View>
        ) : null}

        <View style={styles.actions}>
          <Button title={`Enable ${label}`} onPress={handleEnable} loading={submitting} block />
          <Button title="Not now" onPress={finish} variant="secondary" block />
        </View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 24 },
  card: { width: '100%', maxWidth: 380, borderWidth: 1, padding: 28, alignItems: 'center' },
  iconWrap: { width: 64, height: 64, borderRadius: 32, alignItems: 'center', justifyContent: 'center', marginBottom: 16 },
  title: { fontSize: 19, marginBottom: 8, textAlign: 'center' },
  subtitle: { fontSize: 13, textAlign: 'center', lineHeight: 19, marginBottom: 20 },
  errorBanner: { padding: 12, marginBottom: 16, width: '100%' },
  actions: { width: '100%', gap: 10 },
});
