import { ChevronLeft, ChevronRight, RotateCcw } from 'lucide-react-native';
import React from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, { runOnJS, useAnimatedStyle, useSharedValue, withSpring } from 'react-native-reanimated';

import { RosterStudent } from '../api/types';
import { useTheme } from '../theme/ThemeProvider';
import { initials } from '../utils/initials';
import { IconButton } from './ui/IconButton';

const PULL_THRESHOLD = 32;
const PULL_MAX = 44;
const HANDLE_SIZE = 34;

interface RosterListRowProps {
  student: RosterStudent;
  disabled: boolean;
  onMark: (student: RosterStudent, status: 'present' | 'absent') => void;
  onUndo: (student: RosterStudent) => void;
}

/// A small drag handle pinned to one edge of the row instead of the whole
/// row being swipeable. The row's name/avatar area is plain, undraggable
/// content, so touching it to scroll the list no longer competes with a
/// pan gesture the way a full-row swipe did — only deliberately grabbing
/// this ~34px handle starts a drag. A tap works too, for the same result
/// without needing to pull it at all.
const PullHandle = ({
  side,
  color,
  bg,
  icon,
  enabled,
  onCommit,
  accessibilityLabel,
}: {
  side: 'left' | 'right';
  color: string;
  bg: string;
  icon: React.ReactNode;
  enabled: boolean;
  onCommit: () => void;
  accessibilityLabel: string;
}) => {
  const pull = useSharedValue(0);
  const sign = side === 'left' ? 1 : -1;

  const pan = Gesture.Pan()
    .enabled(enabled)
    .onUpdate((e) => {
      const raw = e.translationX * sign;
      pull.value = Math.max(0, Math.min(raw, PULL_MAX));
    })
    .onEnd(() => {
      if (pull.value > PULL_THRESHOLD) {
        runOnJS(onCommit)();
      }
      pull.value = withSpring(0);
    });

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: pull.value * sign }],
  }));

  return (
    <GestureDetector gesture={pan}>
      <Animated.View style={animatedStyle}>
        <Pressable
          onPress={enabled ? onCommit : undefined}
          disabled={!enabled}
          accessibilityLabel={accessibilityLabel}
          style={({ pressed }) => [
            styles.handle,
            { backgroundColor: bg, opacity: enabled ? (pressed ? 0.8 : 1) : 0.4 },
          ]}
        >
          {icon}
        </Pressable>
      </Animated.View>
    </GestureDetector>
  );
};

export const RosterListRow = ({ student, disabled, onMark, onUndo }: RosterListRowProps) => {
  const { colors, radii, font } = useTheme();

  // A self-submitted row is forensic student data — the manual-mark endpoint
  // 409s on it, so it isn't markable here either; shown read-only instead.
  const readOnly = student.source === 'self_submitted';
  const pullEnabled = !readOnly && !disabled;

  const commitPresent = () => onMark(student, 'present');
  const commitAbsent = () => onMark(student, 'absent');

  const statusColor = student.status === 'present' ? colors.success : student.status === 'absent' ? colors.danger : undefined;
  const borderColor = statusColor ?? colors.border;
  const avatarBg = student.status === 'present' ? colors.successBg : student.status === 'absent' ? colors.dangerBg : colors.primaryLight;
  const avatarFg = student.status === 'present' ? colors.success : student.status === 'absent' ? colors.danger : colors.primary;
  const rollColor = student.status === 'unmarked' ? colors.muted : borderColor;

  return (
    <View
      style={[
        styles.row,
        { backgroundColor: colors.surface, borderColor, borderRadius: radii.md, opacity: readOnly ? 0.7 : 1 },
      ]}
    >
      <PullHandle
        side="left"
        color={colors.success}
        bg={colors.successBg}
        enabled={pullEnabled}
        onCommit={commitPresent}
        accessibilityLabel={`Mark ${student.name} present`}
        icon={<ChevronRight size={18} color={colors.success} />}
      />

      <View style={[styles.avatar, { backgroundColor: avatarBg }]}>
        <Text style={[styles.avatarText, { color: avatarFg, fontFamily: font.extrabold }]}>{initials(student.name)}</Text>
      </View>
      <View style={styles.info}>
        <Text style={[styles.name, { color: statusColor ?? colors.text, fontFamily: font.bold }]} numberOfLines={1}>
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

      <PullHandle
        side="right"
        color={colors.danger}
        bg={colors.dangerBg}
        enabled={pullEnabled}
        onCommit={commitAbsent}
        accessibilityLabel={`Mark ${student.name} absent`}
        icon={<ChevronLeft size={18} color={colors.danger} />}
      />
    </View>
  );
};

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
    paddingVertical: 12,
    paddingHorizontal: 10,
    borderWidth: 1.5,
    marginBottom: 10,
  },
  handle: {
    width: HANDLE_SIZE,
    height: HANDLE_SIZE,
    borderRadius: HANDLE_SIZE / 2,
    alignItems: 'center',
    justifyContent: 'center',
    flexShrink: 0,
  },
  avatar: { width: 38, height: 38, borderRadius: 19, alignItems: 'center', justifyContent: 'center', flexShrink: 0 },
  avatarText: { fontSize: 13 },
  info: { flex: 1, minWidth: 0 },
  name: { fontSize: 14 },
  roll: { fontSize: 12, marginTop: 2 },
});
