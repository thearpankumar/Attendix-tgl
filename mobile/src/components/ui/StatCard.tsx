import type { LucideIcon } from 'lucide-react-native';
import React from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

type Tone = 'primary' | 'success' | 'warning';

interface StatCardProps {
  icon: LucideIcon;
  label: string;
  value: number | string;
  tone: Tone;
}

export const StatCard = ({ icon: Icon, label, value, tone }: StatCardProps) => {
  const { colors, radii, shadows, font } = useTheme();

  const iconBg = tone === 'success' ? colors.successBg : tone === 'warning' ? colors.warningBg : colors.primaryLight;
  const iconColor = tone === 'success' ? colors.success : tone === 'warning' ? colors.warning : colors.primary;

  return (
    <View style={[styles.card, { backgroundColor: colors.surface, borderColor: colors.border, borderRadius: radii.md, ...shadows.card }]}>
      <View style={[styles.iconWrap, { backgroundColor: iconBg }]}>
        <Icon size={17} color={iconColor} />
      </View>
      <View>
        <Text style={[styles.value, { color: colors.text, fontFamily: font.extrabold }]}>{value}</Text>
        <Text style={[styles.label, { color: colors.muted, fontFamily: font.semibold }]}>{label}</Text>
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  card: {
    flexBasis: '48%',
    flexGrow: 1,
    borderWidth: 1,
    padding: 14,
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
  },
  iconWrap: {
    width: 36,
    height: 36,
    borderRadius: 10,
    alignItems: 'center',
    justifyContent: 'center',
  },
  value: { fontSize: 20, lineHeight: 22 },
  label: { fontSize: 11, marginTop: 3 },
});
