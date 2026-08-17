import { LinearGradient } from 'expo-linear-gradient';
import React from 'react';
import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

type Variant = 'primary' | 'secondary' | 'success' | 'danger';

interface ButtonProps {
  title: string;
  onPress: () => void;
  variant?: Variant;
  block?: boolean;
  disabled?: boolean;
  loading?: boolean;
  icon?: React.ReactNode;
}

export const Button = ({ title, onPress, variant = 'primary', block, disabled, loading, icon }: ButtonProps) => {
  const { colors, radii } = useTheme();
  const isDisabled = disabled || loading;

  const solidBg = variant === 'success' ? colors.success : variant === 'danger' ? colors.danger : colors.bgSubtle;
  const textColor = variant === 'secondary' ? colors.text : colors.onPrimary;
  const borderColor = variant === 'secondary' ? colors.border : 'transparent';

  const content = (
    <View style={styles.content}>
      {loading ? <ActivityIndicator size="small" color={textColor} /> : icon}
      <Text style={[styles.text, { color: textColor }]}>{title}</Text>
    </View>
  );

  if (variant === 'primary') {
    return (
      <Pressable
        onPress={onPress}
        disabled={isDisabled}
        style={({ pressed }) => [{ width: block ? '100%' : undefined, opacity: isDisabled ? 0.55 : pressed ? 0.9 : 1 }]}
      >
        <LinearGradient
          colors={[colors.primary, colors.primaryDark]}
          start={{ x: 0, y: 0 }}
          end={{ x: 1, y: 1 }}
          style={[styles.base, { borderRadius: radii.sm }]}
        >
          {content}
        </LinearGradient>
      </Pressable>
    );
  }

  return (
    <Pressable
      onPress={onPress}
      disabled={isDisabled}
      style={({ pressed }) => [
        styles.base,
        {
          backgroundColor: solidBg,
          borderRadius: radii.sm,
          borderWidth: variant === 'secondary' ? 1 : 0,
          borderColor,
          opacity: isDisabled ? 0.55 : pressed ? 0.9 : 1,
          width: block ? '100%' : undefined,
        },
      ]}
    >
      {content}
    </Pressable>
  );
};

const styles = StyleSheet.create({
  base: {
    paddingVertical: 12,
    paddingHorizontal: 20,
    alignItems: 'center',
    justifyContent: 'center',
  },
  content: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  text: {
    fontSize: 15,
    fontWeight: '700',
  },
});
