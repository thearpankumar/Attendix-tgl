import { LinearGradient } from 'expo-linear-gradient';
import React from 'react';
import { StyleSheet, View } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

export const ProgressBar = ({ percent }: { percent: number }) => {
  const { colors, radii } = useTheme();
  const clamped = Math.max(0, Math.min(100, percent));

  return (
    <View style={[styles.track, { backgroundColor: colors.bgSubtle, borderRadius: radii.pill }]}>
      <LinearGradient
        colors={[colors.success, colors.primary]}
        start={{ x: 0, y: 0 }}
        end={{ x: 1, y: 0 }}
        style={[styles.fill, { width: `${clamped}%`, borderRadius: radii.pill }]}
      />
    </View>
  );
};

const styles = StyleSheet.create({
  track: { height: 8, overflow: 'hidden' },
  fill: { height: '100%' },
});
