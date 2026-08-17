import { useRouter } from 'expo-router';
import { ArrowLeft, ClipboardCheck, LogOut, Settings as SettingsIcon } from 'lucide-react-native';
import React from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { useAuth } from '../context/AuthContext';
import { useTheme } from '../theme/ThemeProvider';
import { IconButton } from './ui/IconButton';

export const Topbar = ({ mode }: { mode: 'brand' | 'back' }) => {
  const { colors, font } = useTheme();
  const { admin, logout } = useAuth();
  const router = useRouter();
  const insets = useSafeAreaInsets();

  const displayName = admin?.fullName || admin?.username || '';

  return (
    <View style={[styles.bar, { backgroundColor: colors.surface, borderBottomColor: colors.border, paddingTop: insets.top + 14 }]}>
      {mode === 'back' ? (
        <IconButton accessibilityLabel="Back" onPress={() => router.back()} size={36}>
          <ArrowLeft size={18} color={colors.text} />
        </IconButton>
      ) : (
        <View style={styles.brand}>
          <ClipboardCheck size={20} color={colors.primary} />
          <Text style={[styles.brandText, { color: colors.text, fontFamily: font.extrabold }]}>Attendix</Text>
        </View>
      )}

      <View style={styles.actions}>
        {displayName ? <Text style={[styles.name, { color: colors.muted, fontFamily: font.medium }]}>{displayName}</Text> : null}
        <IconButton accessibilityLabel="Settings" onPress={() => router.push('/settings')}>
          <SettingsIcon size={16} color={colors.muted} />
        </IconButton>
        <IconButton accessibilityLabel="Log out" onPress={() => logout()}>
          <LogOut size={16} color={colors.muted} />
        </IconButton>
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  bar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingBottom: 14,
    borderBottomWidth: 1,
  },
  brand: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  brandText: { fontSize: 17 },
  actions: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  name: { fontSize: 13, marginRight: 2 },
});
