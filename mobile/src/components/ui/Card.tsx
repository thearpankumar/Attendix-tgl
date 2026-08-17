import React from 'react';
import { Pressable, StyleSheet, View, ViewStyle } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

interface CardProps {
  children: React.ReactNode;
  onPress?: () => void;
  style?: ViewStyle;
  padded?: boolean;
}

export const Card = ({ children, onPress, style, padded = true }: CardProps) => {
  const { colors, radii, shadows } = useTheme();
  const base: ViewStyle = {
    backgroundColor: colors.surface,
    borderColor: colors.border,
    borderWidth: 1,
    borderRadius: radii.md,
    padding: padded ? 16 : 0,
    ...shadows.card,
  };

  if (onPress) {
    return (
      <Pressable onPress={onPress} style={({ pressed }) => [base, style, pressed && styles.pressed]}>
        {children}
      </Pressable>
    );
  }

  return <View style={[base, style]}>{children}</View>;
};

const styles = StyleSheet.create({
  pressed: { transform: [{ scale: 0.98 }] },
});
