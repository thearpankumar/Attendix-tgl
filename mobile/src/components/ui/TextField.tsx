import React from 'react';
import { StyleSheet, Text, TextInput, TextInputProps, View } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

interface TextFieldProps extends TextInputProps {
  label: string;
  error?: string;
}

export const TextField = ({ label, error, style, ...inputProps }: TextFieldProps) => {
  const { colors, radii, font } = useTheme();
  const [focused, setFocused] = React.useState(false);

  return (
    <View style={styles.field}>
      <Text style={[styles.label, { color: colors.muted, fontFamily: font.semibold }]}>{label}</Text>
      <TextInput
        placeholderTextColor={colors.faint}
        {...inputProps}
        onFocus={(e) => {
          setFocused(true);
          inputProps.onFocus?.(e);
        }}
        onBlur={(e) => {
          setFocused(false);
          inputProps.onBlur?.(e);
        }}
        style={[
          styles.input,
          {
            borderRadius: radii.sm,
            borderColor: focused ? colors.primary : colors.border,
            backgroundColor: colors.bgSubtle,
            color: colors.text,
            fontFamily: font.regular,
          },
          style,
        ]}
      />
      {error ? <Text style={[styles.error, { color: colors.dangerTxt }]}>{error}</Text> : null}
    </View>
  );
};

const styles = StyleSheet.create({
  field: { marginBottom: 16 },
  label: { fontSize: 13, marginBottom: 6 },
  input: {
    width: '100%',
    paddingVertical: 12,
    paddingHorizontal: 14,
    borderWidth: 1.5,
    fontSize: 15,
  },
  error: { fontSize: 12, marginTop: 6 },
});
