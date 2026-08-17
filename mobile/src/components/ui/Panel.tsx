import React from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

export const Panel = ({ header, children }: { header: string; children: React.ReactNode }) => {
  const { colors, radii, shadows, font } = useTheme();

  return (
    <View style={[styles.panel, { backgroundColor: colors.surface, borderColor: colors.border, borderRadius: radii.lg, ...shadows.card }]}>
      <Text style={[styles.header, { color: colors.muted, borderBottomColor: colors.border, fontFamily: font.bold }]}>{header}</Text>
      {children}
    </View>
  );
};

const styles = StyleSheet.create({
  panel: { borderWidth: 1, marginTop: 16, overflow: 'hidden' },
  header: {
    padding: 14,
    fontSize: 12,
    textTransform: 'uppercase',
    letterSpacing: 0.4,
    borderBottomWidth: 1,
  },
});
