import React from 'react';
import { StyleSheet, Text, View } from 'react-native';
import Svg, { Circle } from 'react-native-svg';

import { useTheme } from '../theme/ThemeProvider';

const SIZE = 92;
const STROKE = 12;
const RADIUS = (SIZE - STROKE) / 2;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export const DonutChart = ({ percent }: { percent: number }) => {
  const { colors, font } = useTheme();
  const clamped = Math.max(0, Math.min(100, percent));
  const presentLength = (clamped / 100) * CIRCUMFERENCE;

  return (
    <View style={{ width: SIZE, height: SIZE }}>
      <Svg width={SIZE} height={SIZE}>
        <Circle cx={SIZE / 2} cy={SIZE / 2} r={RADIUS} stroke={colors.danger} strokeWidth={STROKE} fill="none" />
        <Circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={RADIUS}
          stroke={colors.success}
          strokeWidth={STROKE}
          strokeDasharray={`${presentLength} ${CIRCUMFERENCE}`}
          strokeLinecap="butt"
          fill="none"
          // Conic-gradients (and this arc) start at 12 o'clock, going clockwise.
          rotation={-90}
          origin={`${SIZE / 2}, ${SIZE / 2}`}
        />
      </Svg>
      <View style={styles.center} pointerEvents="none">
        <Text style={[styles.pct, { color: colors.text, fontFamily: font.extrabold }]}>{clamped}%</Text>
        <Text style={[styles.label, { color: colors.muted, fontFamily: font.semibold }]}>Present</Text>
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  center: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    alignItems: 'center',
    justifyContent: 'center',
  },
  pct: { fontSize: 17 },
  label: { fontSize: 9, textTransform: 'uppercase', letterSpacing: 0.4, marginTop: 1 },
});
