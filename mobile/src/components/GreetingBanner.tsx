import { LinearGradient } from 'expo-linear-gradient';
import React from 'react';
import { StyleSheet, Text, View } from 'react-native';
import Svg, { Circle } from 'react-native-svg';

import { useTheme } from '../theme/ThemeProvider';
import { greetingPeriod, greetingWord, GreetingPeriod } from '../utils/greeting';

// Gradient stands in for a time-of-day sky photo — no network fetch, no
// licensing to track, crisp at any size, and small enough to ship in the
// bundle. Each pair starts with a mid/dark tone at the top-left (where the
// greeting text sits) so white text stays legible without relying solely on
// the text shadow below.
const PALETTES: Record<GreetingPeriod, readonly [string, string]> = {
  morning: ['#e2711d', '#4568dc'],
  afternoon: ['#1e6f8e', '#4fb3d9'],
  night: ['#0f0c29', '#33296b'],
};

const SkyArt = ({ period }: { period: GreetingPeriod }) => {
  if (period === 'night') {
    const stars: [number, number][] = [
      [28, 18], [52, 44], [88, 14], [118, 52], [18, 66], [66, 78], [140, 30],
    ];
    return (
      <Svg viewBox="0 0 200 100" width="100%" height="100%" style={StyleSheet.absoluteFill}>
        <Circle cx={158} cy={28} r={16} fill="#fdf3d0" opacity={0.95} />
        <Circle cx={151} cy={22} r={13} fill={PALETTES.night[1]} />
        {stars.map(([cx, cy], i) => (
          <Circle key={i} cx={cx} cy={cy} r={1.4} fill="#ffffff" opacity={0.75} />
        ))}
      </Svg>
    );
  }

  // Morning: low sun, still rising. Afternoon: high sun, full glow.
  const sunY = period === 'morning' ? 72 : 26;
  return (
    <Svg viewBox="0 0 200 100" width="100%" height="100%" style={StyleSheet.absoluteFill}>
      <Circle cx={162} cy={sunY} r={26} fill="#ffffff" opacity={0.18} />
      <Circle cx={162} cy={sunY} r={15} fill="#fff7e0" opacity={0.65} />
    </Svg>
  );
};

/// Home screen's greeting card — time-of-day gradient + sky art behind a
/// simple "Good morning/afternoon/evening" (no name: the Topbar already
/// shows who's logged in, right below the Attendix logo).
export const GreetingBanner = () => {
  const { radii, font } = useTheme();
  const period = greetingPeriod();
  const colors = PALETTES[period];

  return (
    <View style={[styles.wrap, { borderRadius: radii.lg }]}>
      <LinearGradient colors={colors} start={{ x: 0, y: 0 }} end={{ x: 1, y: 1 }} style={StyleSheet.absoluteFill} />
      <SkyArt period={period} />
      <Text style={[styles.greeting, { fontFamily: font.extrabold }]}>{greetingWord()} 👋</Text>
      <Text style={styles.sub}>Here&apos;s what&apos;s happening today</Text>
    </View>
  );
};

const styles = StyleSheet.create({
  wrap: { padding: 18, paddingVertical: 22, overflow: 'hidden', marginBottom: 4, justifyContent: 'center' },
  greeting: {
    fontSize: 22,
    color: '#ffffff',
    textShadowColor: 'rgba(0,0,0,0.35)',
    textShadowOffset: { width: 0, height: 1 },
    textShadowRadius: 3,
  },
  sub: {
    fontSize: 13,
    marginTop: 4,
    color: 'rgba(255,255,255,0.88)',
    textShadowColor: 'rgba(0,0,0,0.3)',
    textShadowOffset: { width: 0, height: 1 },
    textShadowRadius: 2,
  },
});
