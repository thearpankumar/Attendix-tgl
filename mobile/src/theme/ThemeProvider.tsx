import React, { createContext, useContext, useMemo } from 'react';
import { useColorScheme } from 'react-native';

import { darkColors, fontFamily, lightColors, radii, shadows, spacing, ThemeColors } from './tokens';

interface Theme {
  colors: ThemeColors;
  radii: typeof radii;
  shadows: typeof shadows;
  spacing: typeof spacing;
  font: typeof fontFamily;
  isDark: boolean;
}

const ThemeContext = createContext<Theme | null>(null);

export const useTheme = (): Theme => {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used within ThemeProvider');
  return ctx;
};

// Auto light/dark, no manual toggle — matches frontend/mentor (prefers-color-scheme only).
export const ThemeProvider = ({ children }: { children: React.ReactNode }) => {
  const scheme = useColorScheme();
  const isDark = scheme === 'dark';

  const value = useMemo<Theme>(
    () => ({
      colors: isDark ? darkColors : lightColors,
      radii,
      shadows,
      spacing,
      font: fontFamily,
      isDark,
    }),
    [isDark]
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
};
