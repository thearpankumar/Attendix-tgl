import { RotateCcw } from 'lucide-react-native';
import React from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, { runOnJS, useAnimatedStyle, useSharedValue, withSpring } from 'react-native-reanimated';

import { RosterStudent } from '../api/types';
import { useTheme } from '../theme/ThemeProvider';
import { initials } from '../utils/initials';
import { IconButton } from './ui/IconButton';

const SWIPE_THRESHOLD = 90;

interface RosterListRowProps {
  student: RosterStudent;
  disabled: boolean;
  onMark: (student: RosterStudent, status: 'present' | 'absent') => void;
  onUndo: (student: RosterStudent) => void;
}

export const RosterListRow = ({ student, disabled, onMark, onUndo }: RosterListRowProps) => {
  const { colors, radii, font } = useTheme();
  const translateX = useSharedValue(0);

  // A self-submitted row is forensic student data — the manual-mark endpoint
  // 409s on it, so it isn't draggable here either; shown read-only instead.
  const readOnly = student.source === 'self_submitted';
  const swipeEnabled = !readOnly && !disabled;

  const commitPresent = () => onMark(student, 'present');
  const commitAbsent = () => onMark(student, 'absent');

  const pan = Gesture.Pan()
    .enabled(swipeEnabled)
    .onUpdate((e) => {
      translateX.value = e.translationX;
    })
    .onEnd((e) => {
      if (e.translationX < -SWIPE_THRESHOLD) {
        runOnJS(commitPresent)();
      } else if (e.translationX > SWIPE_THRESHOLD) {
        runOnJS(commitAbsent)();
      }
      translateX.value = withSpring(0);
    });

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: translateX.value }],
  }));

  const borderColor = student.status === 'present' ? colors.success : student.status === 'absent' ? colors.danger : colors.border;
  const avatarBg = student.status === 'present' ? colors.successBg : student.status === 'absent' ? colors.dangerBg : colors.primaryLight;
  const avatarFg = student.status === 'present' ? colors.success : student.status === 'absent' ? colors.danger : colors.primary;
  const rollColor = student.status === 'unmarked' ? colors.muted : borderColor;

  return (
    <GestureDetector gesture={pan}>
      <Animated.View
        style={[
          styles.row,
          animatedStyle,
          { backgroundColor: colors.surface, borderColor, borderRadius: radii.md, opacity: readOnly ? 0.7 : 1 },
        ]}
      >
        <View style={[styles.avatar, { backgroundColor: avatarBg }]}>
          <Text style={[styles.avatarText, { color: avatarFg, fontFamily: font.extrabold }]}>{initials(student.name)}</Text>
        </View>
        <View style={styles.info}>
          <Text style={[styles.name, { color: colors.text, fontFamily: font.bold }]} numberOfLines={1}>
            {student.name}
          </Text>
          <Text style={[styles.roll, { color: rollColor }]} numberOfLines={1}>
            {student.rollNumber}
            {student.source === 'self_submitted' ? ' · Self-submitted' : ''}
          </Text>
        </View>
        {student.source === 'manual' ? (
          <IconButton accessibilityLabel={`Undo mark for ${student.name}`} onPress={() => onUndo(student)} size={30}>
            <RotateCcw size={13} color={colors.muted} />
          </IconButton>
        ) : null}
      </Animated.View>
    </GestureDetector>
  );
};

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    paddingVertical: 12,
    paddingHorizontal: 14,
    borderWidth: 1.5,
    marginBottom: 10,
  },
  avatar: { width: 38, height: 38, borderRadius: 19, alignItems: 'center', justifyContent: 'center', flexShrink: 0 },
  avatarText: { fontSize: 13 },
  info: { flex: 1, minWidth: 0 },
  name: { fontSize: 14 },
  roll: { fontSize: 12, marginTop: 2 },
});
