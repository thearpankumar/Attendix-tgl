import React from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, { runOnJS, useAnimatedStyle, useSharedValue, withSpring, withTiming } from 'react-native-reanimated';

import { RosterStudent } from '../api/types';
import { useTheme } from '../theme/ThemeProvider';
import { initials } from '../utils/initials';

const SWIPE_THRESHOLD = 90;
const EXIT_DISTANCE = 500;

interface SwipeStackCardProps {
  student: RosterStudent;
  isTop: boolean;
  onCommit: (status: 'present' | 'absent') => void;
}

const SwipeStackCard = ({ student, isTop, onCommit }: SwipeStackCardProps) => {
  const { colors, radii, shadows, font } = useTheme();
  const translateX = useSharedValue(0);

  const commit = (status: 'present' | 'absent') => onCommit(status);

  const pan = Gesture.Pan()
    .enabled(isTop)
    .onUpdate((e) => {
      translateX.value = e.translationX;
    })
    .onEnd((e) => {
      if (e.translationX < -SWIPE_THRESHOLD) {
        translateX.value = withTiming(-EXIT_DISTANCE, { duration: 220 }, (finished) => {
          if (finished) runOnJS(commit)('present');
        });
      } else if (e.translationX > SWIPE_THRESHOLD) {
        translateX.value = withTiming(EXIT_DISTANCE, { duration: 220 }, (finished) => {
          if (finished) runOnJS(commit)('absent');
        });
      } else {
        translateX.value = withSpring(0);
      }
    });

  // isTop is fixed for the lifetime of a mounted card (cards remount on key
  // change as the roster's unmarked list shifts), so baking it into the
  // worklet closure directly is safe and avoids fighting RN's style-array
  // merge semantics with a second, conditionally-undefined transform.
  const animatedStyle = useAnimatedStyle(() => ({
    transform: [
      { translateX: translateX.value },
      { translateY: isTop ? 0 : 10 },
      { scale: isTop ? 1 : 0.96 },
      { rotate: `${translateX.value / 20}deg` },
    ],
  }));

  return (
    <GestureDetector gesture={pan}>
      <Animated.View
        style={[
          styles.card,
          animatedStyle,
          {
            backgroundColor: colors.surface,
            borderColor: colors.border,
            borderRadius: radii.lg,
            zIndex: isTop ? 2 : 1,
            ...shadows.lg,
          },
        ]}
      >
        <View style={[styles.avatar, { backgroundColor: colors.primaryLight }]}>
          <Text style={[styles.avatarText, { color: colors.primary, fontFamily: font.extrabold }]}>{initials(student.name)}</Text>
        </View>
        <Text style={[styles.name, { color: colors.text, fontFamily: font.bold }]}>{student.name}</Text>
        <Text style={[styles.roll, { color: colors.muted }]}>{student.rollNumber}</Text>
      </Animated.View>
    </GestureDetector>
  );
};

interface SwipeStackProps {
  students: RosterStudent[];
  onMark: (student: RosterStudent, status: 'present' | 'absent') => void;
}

export const SwipeStack = ({ students, onMark }: SwipeStackProps) => {
  const top2 = students.slice(0, 2);
  const ordered = top2.slice().reverse();

  return (
    <View style={styles.stage}>
      {ordered.map((student, idx) => {
        const isTop = idx === ordered.length - 1;
        return <SwipeStackCard key={student.rollNumber} student={student} isTop={isTop} onCommit={(status) => onMark(student, status)} />;
      })}
    </View>
  );
};

const styles = StyleSheet.create({
  stage: { height: 320, marginVertical: 12 },
  card: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    borderWidth: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 24,
  },
  avatar: { width: 64, height: 64, borderRadius: 32, alignItems: 'center', justifyContent: 'center', marginBottom: 14 },
  avatarText: { fontSize: 22 },
  name: { fontSize: 19 },
  roll: { fontSize: 14, marginTop: 2 },
});
