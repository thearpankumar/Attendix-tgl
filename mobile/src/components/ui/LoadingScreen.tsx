import React from 'react';
import { ActivityIndicator, StyleSheet, View } from 'react-native';

import { useTheme } from '../../theme/ThemeProvider';

export const LoadingScreen = ({ fullScreen = true }: { fullScreen?: boolean }) => {
  const { colors } = useTheme();

  return (
    <View style={[styles.wrap, { backgroundColor: colors.bg, minHeight: fullScreen ? '100%' : 200 }]}>
      <ActivityIndicator size="large" color={colors.primary} />
    </View>
  );
};

const styles = StyleSheet.create({
  wrap: { alignItems: 'center', justifyContent: 'center' },
});
