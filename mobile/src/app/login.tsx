import { zodResolver } from '@hookform/resolvers/zod';
import { useRouter } from 'expo-router';
import React, { useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { KeyboardAvoidingView, Platform, ScrollView, StyleSheet, Text, View } from 'react-native';
import Svg, { Circle, Polygon, Rect } from 'react-native-svg';
import { z } from 'zod';

import { useAuth } from '../context/AuthContext';
import { useBiometricLock } from '../context/BiometricLockContext';
import { useTheme } from '../theme/ThemeProvider';
import { Button } from '../components/ui/Button';
import { TextField } from '../components/ui/TextField';

const schema = z.object({
  username: z.string().min(1, 'Username is required'),
  password: z.string().min(1, 'Password is required'),
});

type FormValues = z.infer<typeof schema>;

const CampusIllustration = ({ primary, primaryLight, primaryDark, border, surface }: Record<string, string>) => (
  <Svg viewBox="0 0 240 150" width="100%" height={120}>
    <Circle cx={120} cy={95} r={70} fill={primaryLight} />
    <Circle cx={36} cy={112} r={16} fill={primaryLight} />
    <Circle cx={206} cy={108} r={12} fill={primaryLight} />
    <Rect x={188} y={70} width={3} height={42} rx={1.5} fill={border} />
    <Circle cx={189.5} cy={66} r={7} fill={primaryLight} stroke={primary} strokeWidth={1.5} />
    <Rect x={30} y={112} width={180} height={4} rx={2} fill={border} />
    <Polygon points="120,28 168,66 72,66" fill={primary} opacity={0.85} />
    <Rect x={66} y={66} width={108} height={10} fill={primaryDark} opacity={0.9} />
    <Rect x={60} y={76} width={120} height={36} fill={surface} stroke={primary} strokeWidth={1.5} />
    {[74, 96, 118, 140, 162].map((x) => (
      <Rect key={x} x={x} y={80} width={8} height={28} fill={primary} opacity={0.75} />
    ))}
    <Rect x={54} y={112} width={132} height={8} fill={primaryDark} opacity={0.9} />
  </Svg>
);

export default function Login() {
  const { colors, radii, shadows, font } = useTheme();
  const { login } = useAuth();
  const biometricLock = useBiometricLock();
  const router = useRouter();
  const [serverError, setServerError] = useState('');

  const { control, handleSubmit, formState: { isSubmitting } } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { username: '', password: '' },
  });

  const onSubmit = async (values: FormValues) => {
    setServerError('');
    const result = await login(values.username, values.password);
    if (!result.success) {
      setServerError(result.message || 'Login failed');
      return;
    }

    const capability = await biometricLock.refreshCapability();
    if (capability.hasHardware && capability.isEnrolled && !biometricLock.enabled) {
      router.replace('/enroll-biometric');
    } else {
      router.replace('/');
    }
  };

  return (
    <KeyboardAvoidingView
      style={[styles.screen, { backgroundColor: colors.bg }]}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
    >
      <ScrollView contentContainerStyle={styles.scroll} keyboardShouldPersistTaps="handled">
        <View
          style={[
            styles.card,
            { backgroundColor: colors.surface, borderColor: colors.border, borderRadius: radii.lg, ...shadows.lg },
          ]}
        >
          <View style={[styles.illustrationWrap, { backgroundColor: colors.primaryLight }]}>
            <CampusIllustration
              primary={colors.primary}
              primaryLight={colors.surface}
              primaryDark={colors.primaryDark}
              border={colors.border}
              surface={colors.surface}
            />
            <Text style={[styles.title, { color: colors.text, fontFamily: font.extrabold }]}>Exam Attendance Portal</Text>
            <Text style={[styles.subtitle, { color: colors.muted, fontFamily: font.regular }]}>Please login to continue</Text>
          </View>

          <View style={styles.form}>
            <Controller
              control={control}
              name="username"
              render={({ field: { onChange, onBlur, value }, fieldState: { error } }) => (
                <TextField
                  label="Username"
                  value={value}
                  onChangeText={onChange}
                  onBlur={onBlur}
                  autoFocus
                  autoCapitalize="none"
                  autoComplete="username"
                  error={error?.message}
                />
              )}
            />
            <Controller
              control={control}
              name="password"
              render={({ field: { onChange, onBlur, value }, fieldState: { error } }) => (
                <TextField
                  label="Password"
                  value={value}
                  onChangeText={onChange}
                  onBlur={onBlur}
                  secureTextEntry
                  autoComplete="current-password"
                  error={error?.message}
                />
              )}
            />

            {serverError ? (
              <View style={[styles.errorBanner, { backgroundColor: colors.dangerBg, borderRadius: radii.sm }]}>
                <Text style={{ color: colors.dangerTxt, fontSize: 13 }}>{serverError}</Text>
              </View>
            ) : null}

            <Button title={isSubmitting ? 'Signing in…' : 'Sign In'} onPress={handleSubmit(onSubmit)} loading={isSubmitting} block />
          </View>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  scroll: { flexGrow: 1, alignItems: 'center', justifyContent: 'center', padding: 24 },
  card: { width: '100%', maxWidth: 380, borderWidth: 1, overflow: 'hidden' },
  illustrationWrap: { paddingTop: 24, paddingBottom: 20, paddingHorizontal: 20, alignItems: 'center' },
  title: { fontSize: 19, marginTop: 12, marginBottom: 4 },
  subtitle: { fontSize: 13 },
  form: { padding: 28 },
  errorBanner: { padding: 12, marginBottom: 16 },
});
