import React from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

type Tone = 'success' | 'danger' | 'warning' | 'neutral';

export const Badge = ({ label, tone = 'neutral' }: { label: string; tone?: Tone }) => {
  const { colors, radii, font } = useTheme();

  const bg =
    tone === 'success' ? colors.successBg : tone === 'danger' ? colors.dangerBg : tone === 'warning' ? colors.warningBg : colors.bgSubtle;
  const fg = tone === 'success' ? colors.successTxt : tone === 'danger' ? colors.dangerTxt : tone === 'warning' ? colors.warningTxt : colors.muted;

  return (
    <View style={[styles.base, { backgroundColor: bg, borderRadius: radii.pill }]}>
      <Text style={[styles.text, { color: fg, fontFamily: font.bold }]}>{label}</Text>
    </View>
  );
};

const styles = StyleSheet.create({
  base: { paddingVertical: 3, paddingHorizontal: 10, alignSelf: 'flex-start' },
  text: { fontSize: 11, textTransform: 'uppercase', letterSpacing: 0.3 },
});
