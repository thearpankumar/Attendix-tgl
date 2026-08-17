import React from 'react';
import { RefreshControl, ScrollView, StyleSheet, View } from 'react-native';

import { useTheme } from '../theme/ThemeProvider';
import { Topbar } from './Topbar';

interface ScreenProps {
  mode: 'brand' | 'back';
  children: React.ReactNode;
  refreshing?: boolean;
  onRefresh?: () => void;
}

export const Screen = ({ mode, children, refreshing, onRefresh }: ScreenProps) => {
  const { colors } = useTheme();

  return (
    <View style={[styles.shell, { backgroundColor: colors.bg }]}>
      <Topbar mode={mode} />
      <ScrollView
        contentContainerStyle={styles.content}
        refreshControl={onRefresh ? <RefreshControl refreshing={!!refreshing} onRefresh={onRefresh} tintColor={colors.primary} /> : undefined}
      >
        <View style={styles.inner}>{children}</View>
      </ScrollView>
    </View>
  );
};

const styles = StyleSheet.create({
  shell: { flex: 1 },
  content: { flexGrow: 1, alignItems: 'center' },
  inner: { width: '100%', maxWidth: 560, padding: 16, paddingBottom: 40 },
});
