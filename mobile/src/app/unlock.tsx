import { zodResolver } from '@hookform/resolvers/zod';
import { Fingerprint } from 'lucide-react-native';
import React, { useEffect, useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { StyleSheet, Text, View } from 'react-native';
import { z } from 'zod';

import { Button } from '../components/ui/Button';
import { TextField } from '../components/ui/TextField';
import { useAuth } from '../context/AuthContext';
import { useBiometricLock } from '../context/BiometricLockContext';
import { useTheme } from '../theme/ThemeProvider';
import { initials } from '../utils/initials';

const schema = z.object({
  username: z.string().min(1, 'Username is required'),
  password: z.string().min(1, 'Password is required'),
});

type FormValues = z.infer<typeof schema>;

export default function Unlock() {
  const { colors, radii, font } = useTheme();
  const { admin, login } = useAuth();
  const biometricLock = useBiometricLock();
  const [usePassword, setUsePassword] = useState(false);
  const [attempted, setAttempted] = useState(false);
  const [error, setError] = useState('');

  const label = biometricLock.capability?.label ?? 'biometric unlock';
  const displayName = admin?.fullName || admin?.username || '';

  const { control, handleSubmit, formState: { isSubmitting } } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { username: admin?.username ?? '', password: '' },
  });

  const attemptUnlock = async () => {
    setError('');
    const success = await biometricLock.unlock();
    setAttempted(true);
    if (!success) setError(`${label} unlock failed or was cancelled.`);
  };

  useEffect(() => {
    // Auto-triggers the native biometric prompt once on mount (an
    // on-mount interaction with an external system — the OS biometric
    // sensor — not a render synchronization), matching the RailOne/PhonePe
    // pattern of prompting immediately rather than waiting for a tap.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    attemptUnlock();
    // Intentionally run once on mount only — attemptUnlock is stable enough
    // for this purpose and re-running it on every render would re-trigger
    // the biometric prompt unexpectedly.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onSubmitPassword = async (values: FormValues) => {
    setError('');
    const result = await login(values.username, values.password);
    if (!result.success) {
      setError(result.message || 'Login failed');
      return;
    }
    biometricLock.unlockWithoutBiometric();
  };

  return (
    <View style={[styles.screen, { backgroundColor: colors.bg }]}>
      <View style={[styles.card, { backgroundColor: colors.surface, borderColor: colors.border, borderRadius: radii.lg }]}>
        <View style={[styles.avatar, { backgroundColor: colors.primaryLight }]}>
          <Text style={[styles.avatarText, { color: colors.primary, fontFamily: font.extrabold }]}>{initials(displayName)}</Text>
        </View>
        <Text style={[styles.title, { color: colors.text, fontFamily: font.extrabold }]}>
          Welcome back{displayName ? `, ${displayName}` : ''}
        </Text>

        {!usePassword ? (
          <>
            <Text style={[styles.subtitle, { color: colors.muted }]}>Unlock Attendix to continue</Text>

            {error ? (
              <View style={[styles.errorBanner, { backgroundColor: colors.dangerBg, borderRadius: radii.sm }]}>
                <Text style={{ color: colors.dangerTxt, fontSize: 13 }}>{error}</Text>
              </View>
            ) : null}

            <View style={styles.actions}>
              <Button title={`Unlock with ${label}`} onPress={attemptUnlock} icon={<Fingerprint size={16} color={colors.onPrimary} />} block />
              {attempted ? (
                <Button title="Use password instead" onPress={() => setUsePassword(true)} variant="secondary" block />
              ) : null}
            </View>
          </>
        ) : (
          <View style={styles.form}>
            <Controller
              control={control}
              name="username"
              render={({ field: { onChange, onBlur, value }, fieldState: { error: fieldError } }) => (
                <TextField label="Username" value={value} onChangeText={onChange} onBlur={onBlur} autoCapitalize="none" error={fieldError?.message} />
              )}
            />
            <Controller
              control={control}
              name="password"
              render={({ field: { onChange, onBlur, value }, fieldState: { error: fieldError } }) => (
                <TextField label="Password" value={value} onChangeText={onChange} onBlur={onBlur} secureTextEntry autoFocus error={fieldError?.message} />
              )}
            />

            {error ? (
              <View style={[styles.errorBanner, { backgroundColor: colors.dangerBg, borderRadius: radii.sm }]}>
                <Text style={{ color: colors.dangerTxt, fontSize: 13 }}>{error}</Text>
              </View>
            ) : null}

            <View style={styles.actions}>
              <Button title={isSubmitting ? 'Signing in…' : 'Sign In'} onPress={handleSubmit(onSubmitPassword)} loading={isSubmitting} block />
              <Button title={`Use ${label} instead`} onPress={() => setUsePassword(false)} variant="secondary" block />
            </View>
          </View>
        )}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 24 },
  card: { width: '100%', maxWidth: 380, borderWidth: 1, padding: 28, alignItems: 'center' },
  avatar: { width: 64, height: 64, borderRadius: 32, alignItems: 'center', justifyContent: 'center', marginBottom: 16 },
  avatarText: { fontSize: 22 },
  title: { fontSize: 19, marginBottom: 6, textAlign: 'center' },
  subtitle: { fontSize: 13, marginBottom: 20, textAlign: 'center' },
  errorBanner: { padding: 12, marginBottom: 16, width: '100%' },
  actions: { width: '100%', gap: 10 },
  form: { width: '100%' },
});
