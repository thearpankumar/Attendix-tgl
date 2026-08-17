import React from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';
import { Card } from './Card';

export const EmptyState = ({ icon, title, subtitle }: { icon?: React.ReactNode; title: string; subtitle?: string }) => {
  const { colors } = useTheme();

  return (
    <Card style={styles.card}>
      <View style={styles.content}>
        {icon}
        <Text style={[styles.title, { color: colors.text }]}>{title}</Text>
        {subtitle ? <Text style={[styles.subtitle, { color: colors.muted }]}>{subtitle}</Text> : null}
      </View>
    </Card>
  );
};

const styles = StyleSheet.create({
  card: { marginTop: 20 },
  content: { alignItems: 'center', paddingVertical: 32, paddingHorizontal: 20, gap: 8 },
  title: { fontSize: 14, textAlign: 'center' },
  subtitle: { fontSize: 13, textAlign: 'center' },
});
