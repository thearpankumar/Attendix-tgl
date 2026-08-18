import { zodResolver } from '@hookform/resolvers/zod';
import { ChevronDown, ChevronUp, Download, Fingerprint, Lock } from 'lucide-react-native';
import React, { useEffect, useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { Pressable, Switch, Text, View, StyleSheet } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';
import Toast from 'react-native-toast-message';
import { z } from 'zod';

import { Screen } from '../../components/Screen';
import { UpdateDetails } from '../../components/UpdateDetails';
import { Button } from '../../components/ui/Button';
import { Card } from '../../components/ui/Card';
import { TextField } from '../../components/ui/TextField';
import { useAuth } from '../../context/AuthContext';
import { useBiometricLock } from '../../context/BiometricLockContext';
import { useAppUpdate } from '../../hooks/useAppUpdate';
import { useTheme } from '../../theme/ThemeProvider';

const schema = z
  .object({
    currentPassword: z.string().min(1, 'Current password is required'),
    newPassword: z.string().min(6, 'New password must be at least 6 characters'),
    confirmPassword: z.string().min(1, 'Please confirm your new password'),
  })
  .refine((data) => data.newPassword === data.confirmPassword, {
    message: 'New passwords do not match',
    path: ['confirmPassword'],
  });

type FormValues = z.infer<typeof schema>;

export default function Settings() {
  const { colors, font } = useTheme();
  const { admin, changePassword } = useAuth();
  const biometricLock = useBiometricLock();
  const [biometricBusy, setBiometricBusy] = useState(false);
  const update = useAppUpdate();

  const [passwordOpen, setPasswordOpen] = useState(false);
  const passwordOpenProgress = useSharedValue(0);
  useEffect(() => {
    passwordOpenProgress.value = withTiming(passwordOpen ? 1 : 0, { duration: 200 });
  }, [passwordOpen, passwordOpenProgress]);
  const passwordFieldsStyle = useAnimatedStyle(() => ({
    maxHeight: passwordOpenProgress.value * 400,
    opacity: passwordOpenProgress.value,
  }));

  const { control, handleSubmit, reset, formState: { isSubmitting } } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { currentPassword: '', newPassword: '', confirmPassword: '' },
  });

  const onSubmit = async (values: FormValues) => {
    const result = await changePassword(values.currentPassword, values.newPassword);
    if (result.success) {
      Toast.show({ type: 'success', text1: 'Password updated' });
      reset();
    } else {
      Toast.show({ type: 'error', text1: result.message || 'Failed to update password' });
    }
  };

  const onToggleBiometric = async (next: boolean) => {
    setBiometricBusy(true);
    try {
      if (next) {
        const result = await biometricLock.enroll();
        if (!result.success && result.message) {
          Toast.show({ type: 'error', text1: result.message });
        }
      } else {
        await biometricLock.disable();
      }
    } catch {
      Toast.show({ type: 'error', text1: 'An unexpected error occurred.' });
    } finally {
      setBiometricBusy(false);
    }
  };

  const biometricLabel = biometricLock.capability?.label ?? 'Biometric unlock';
  const biometricAvailable = !!biometricLock.capability?.hasHardware && !!biometricLock.capability?.isEnrolled;

  return (
    <Screen mode="back">
      <Text style={[styles.title, { color: colors.text, fontFamily: font.extrabold }]}>Settings</Text>
      <Text style={[styles.subtitle, { color: colors.muted }]}>
        Signed in as <Text style={{ fontWeight: '700' }}>{admin?.fullName || admin?.username}</Text> ({admin?.email})
        {admin?.collegeName ? ` · ${admin.collegeName}` : ''}
      </Text>

      <Card style={{ padding: 16, marginBottom: 16 }}>
        <View style={styles.rowHeader}>
          <Fingerprint size={16} color={colors.primary} />
          <Text style={[styles.rowHeaderText, { color: colors.text, fontFamily: font.bold }]}>{biometricLabel} Unlock</Text>
        </View>
        {biometricAvailable ? (
          <View style={styles.switchRow}>
            <Text style={{ color: colors.muted, fontSize: 13, flex: 1 }}>
              Unlock Attendix with {biometricLabel.toLowerCase()} instead of your password.
            </Text>
            <Switch value={biometricLock.enabled} onValueChange={onToggleBiometric} disabled={biometricBusy} />
          </View>
        ) : (
          <Text style={{ color: colors.muted, fontSize: 13 }}>
            No {biometricLabel.toLowerCase()} set up on this device. Enroll it in your phone&apos;s Settings to use quick unlock here.
          </Text>
        )}
      </Card>

      <Card style={{ padding: 16, marginBottom: 16 }}>
        <Pressable
          style={styles.rowHeader}
          onPress={() => setPasswordOpen((open) => !open)}
          accessibilityRole="button"
          accessibilityLabel="Change Password"
        >
          <Lock size={16} color={colors.primary} />
          <Text style={[styles.rowHeaderText, { color: colors.text, fontFamily: font.bold, flex: 1 }]}>Change Password</Text>
          {passwordOpen ? <ChevronUp size={16} color={colors.muted} /> : <ChevronDown size={16} color={colors.muted} />}
        </Pressable>

        <Animated.View style={[{ overflow: 'hidden' }, passwordFieldsStyle]}>
          <Controller
            control={control}
            name="currentPassword"
            render={({ field: { onChange, onBlur, value }, fieldState: { error } }) => (
              <TextField label="Current Password" value={value} onChangeText={onChange} onBlur={onBlur} secureTextEntry error={error?.message} />
            )}
          />
          <Controller
            control={control}
            name="newPassword"
            render={({ field: { onChange, onBlur, value }, fieldState: { error } }) => (
              <TextField label="New Password" value={value} onChangeText={onChange} onBlur={onBlur} secureTextEntry error={error?.message} />
            )}
          />
          <Controller
            control={control}
            name="confirmPassword"
            render={({ field: { onChange, onBlur, value }, fieldState: { error } }) => (
              <TextField label="Confirm New Password" value={value} onChangeText={onChange} onBlur={onBlur} secureTextEntry error={error?.message} />
            )}
          />

          <Button title={isSubmitting ? 'Saving…' : 'Update Password'} onPress={handleSubmit(onSubmit)} loading={isSubmitting} block />
        </Animated.View>
      </Card>

      <Card style={{ padding: 16 }}>
        <View style={styles.rowHeader}>
          <Download size={16} color={colors.primary} />
          <Text style={[styles.rowHeaderText, { color: colors.text, fontFamily: font.bold }]}>App Update</Text>
        </View>

        <Text style={{ color: colors.muted, fontSize: 13, marginBottom: 12 }}>
          Current version: {update.currentVersion}
        </Text>

        {update.updateAvailable && update.latestVersion ? (
          <UpdateDetails
            currentVersion={update.currentVersion}
            latestVersion={update.latestVersion}
            notes={update.notes}
            downloading={update.downloading}
            progress={update.progress}
            error={update.error}
            onUpdate={update.download}
          />
        ) : (
          <>
            {update.hasChecked ? (
              <Text style={{ color: colors.successTxt, fontSize: 13, marginBottom: 12 }}>You&apos;re up to date.</Text>
            ) : null}
            <Button
              title={update.checking ? 'Checking…' : 'Check for Updates'}
              onPress={update.check}
              loading={update.checking}
              variant="secondary"
              block
            />
            {update.error ? <Text style={{ color: colors.dangerTxt, fontSize: 12, marginTop: 8 }}>{update.error}</Text> : null}
          </>
        )}
      </Card>
    </Screen>
  );
}

const styles = StyleSheet.create({
  title: { fontSize: 21, marginBottom: 4 },
  subtitle: { fontSize: 13, marginBottom: 20 },
  rowHeader: { flexDirection: 'row', alignItems: 'center', gap: 8, marginBottom: 16 },
  rowHeaderText: { fontSize: 15 },
  switchRow: { flexDirection: 'row', alignItems: 'center', gap: 12 },
});
