import React from 'react';
import { Pressable, StyleSheet } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

interface IconButtonProps {
  onPress: () => void;
  children: React.ReactNode;
  size?: number;
  accessibilityLabel: string;
}

export const IconButton = ({ onPress, children, size = 38, accessibilityLabel }: IconButtonProps) => {
  const { colors, radii } = useTheme();

  return (
    <Pressable
      onPress={onPress}
      accessibilityLabel={accessibilityLabel}
      style={({ pressed }) => [
        styles.base,
        {
          width: size,
          height: size,
          borderRadius: radii.sm,
          borderColor: colors.border,
          backgroundColor: pressed ? colors.surfaceHover : colors.surface,
        },
      ]}
    >
      {children}
    </Pressable>
  );
};

const styles = StyleSheet.create({
  base: {
    alignItems: 'center',
    justifyContent: 'center',
    borderWidth: 1,
  },
});
