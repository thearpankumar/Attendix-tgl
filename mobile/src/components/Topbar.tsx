import { useRouter } from 'expo-router';
import { ArrowLeft, LogOut, Settings as SettingsIcon } from 'lucide-react-native';
import React from 'react';
import { Image, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { useAuth } from '../context/AuthContext';
import { useTheme } from '../theme/ThemeProvider';
import { IconButton } from './ui/IconButton';

// Same indigo the native splash screen uses (app.json's expo-splash-screen
// plugin) — fixed rather than theme-driven so the brand bar always matches
// the launch screen exactly, in both light and dark app themes.
const BRAND_BG = '#4f46e5';

export const Topbar = ({ mode }: { mode: 'brand' | 'back' }) => {
  const { colors, font } = useTheme();
  const { admin, logout } = useAuth();
  const router = useRouter();
  const insets = useSafeAreaInsets();
  const isBrand = mode === 'brand';

  const displayName = admin?.fullName || admin?.username || '';

  return (
    <View
      style={[
        styles.bar,
        {
          backgroundColor: isBrand ? BRAND_BG : colors.surface,
          borderBottomColor: isBrand ? BRAND_BG : colors.border,
          paddingTop: insets.top + 14,
        },
      ]}
    >
      {isBrand ? (
        <View style={styles.brand}>
          <Image source={require('../../assets/images/splash-icon.png')} style={styles.brandLogo} resizeMode="contain" />
          <Text style={[styles.brandText, { color: '#ffffff', fontFamily: font.extrabold }]}>Attendix</Text>
        </View>
      ) : (
        <IconButton accessibilityLabel="Back" onPress={() => router.back()} size={36}>
          <ArrowLeft size={18} color={colors.text} />
        </IconButton>
      )}

      <View style={styles.actions}>
        {displayName ? (
          <Text style={[styles.name, { color: isBrand ? '#ffffffcc' : colors.muted, fontFamily: font.medium }]}>{displayName}</Text>
        ) : null}
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
  brandLogo: { width: 22, height: 22 },
  brandText: { fontSize: 17 },
  actions: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  name: { fontSize: 13, marginRight: 2 },
});
