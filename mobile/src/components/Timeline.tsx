import React from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { useTheme } from '../theme/ThemeProvider';

export interface TimelineEntry {
  key: string;
  time: string;
  title: string;
  live: boolean;
  upcoming: boolean;
}

export const Timeline = ({ entries }: { entries: TimelineEntry[] }) => {
  const { colors, font } = useTheme();

  return (
    <View style={styles.wrap}>
      {entries.map((entry) => (
        <View key={entry.key} style={styles.item}>
          <View style={styles.marker}>
            <View
              style={[
                styles.dot,
                {
                  backgroundColor: entry.live ? colors.success : colors.surface,
                  borderColor: entry.live ? colors.success : colors.faint,
                },
              ]}
            />
          </View>
          <View style={styles.textWrap}>
            <Text style={[styles.time, { color: colors.muted, fontFamily: font.bold }]}>
              {entry.time}
              {entry.upcoming ? ' · Upcoming' : ''}
            </Text>
            <Text style={[styles.title, { color: colors.text, fontFamily: font.semibold }]}>{entry.title}</Text>
          </View>
        </View>
      ))}
    </View>
  );
};

const styles = StyleSheet.create({
  wrap: { padding: 16, gap: 18 },
  item: { flexDirection: 'row', alignItems: 'flex-start', gap: 12 },
  marker: { paddingTop: 5 },
  dot: { width: 10, height: 10, borderRadius: 5, borderWidth: 2 },
  textWrap: { flex: 1 },
  time: { fontSize: 11, textTransform: 'uppercase', letterSpacing: 0.3 },
  title: { fontSize: 14, marginTop: 2 },
});
