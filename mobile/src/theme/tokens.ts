// Ported 1:1 from frontend/mentor/src/index.css (@theme block + .dark overrides).
// Keep these two palettes in sync with that file if the web theme changes.

export interface ThemeColors {
  primary: string;
  primaryDark: string;
  primaryLight: string;
  success: string;
  successBg: string;
  successTxt: string;
  danger: string;
  dangerBg: string;
  dangerTxt: string;
  warning: string;
  warningBg: string;
  warningTxt: string;
  bg: string;
  bgSubtle: string;
  surface: string;
  surfaceHover: string;
  border: string;
  text: string;
  muted: string;
  faint: string;
  onPrimary: string;
}

export const lightColors: ThemeColors = {
  primary: '#4f46e5',
  primaryDark: '#4338ca',
  primaryLight: '#eef2ff',
  success: '#12b76a',
  successBg: '#ecfdf3',
  successTxt: '#027a48',
  danger: '#f04438',
  dangerBg: '#fef3f2',
  dangerTxt: '#b42318',
  warning: '#f79009',
  warningBg: '#fffaeb',
  warningTxt: '#b54708',
  bg: '#f7f8fb',
  bgSubtle: '#f0f2f7',
  surface: '#ffffff',
  surfaceHover: '#fafbfd',
  border: '#e6e8f0',
  text: '#101828',
  muted: '#667085',
  faint: '#98a2b3',
  onPrimary: '#ffffff',
};

export const darkColors: ThemeColors = {
  primary: '#818cf8',
  primaryDark: '#6366f1',
  primaryLight: '#1e2050',
  success: '#32d583',
  successBg: '#082c1e',
  successTxt: '#32d583',
  danger: '#f97066',
  dangerBg: '#2d1114',
  dangerTxt: '#f97066',
  warning: '#f79009',
  warningBg: '#2b1e02',
  warningTxt: '#fdb022',
  bg: '#0b0d17',
  bgSubtle: '#10131f',
  surface: '#13162a',
  surfaceHover: '#191d35',
  border: '#232748',
  text: '#f0f1f8',
  muted: '#8b90b8',
  faint: '#5c6080',
  onPrimary: '#ffffff',
};

export const radii = {
  sm: 10,
  md: 14,
  lg: 20,
  pill: 999,
};

// react-native's shadow model differs from CSS box-shadow (single
// offset/radius/opacity, no multi-layer stacking), so these approximate the
// web's --shadow-card/-md/-lg rather than reproducing them exactly.
export const shadows = {
  card: {
    shadowColor: '#101828',
    shadowOffset: { width: 0, height: 1 },
    shadowOpacity: 0.06,
    shadowRadius: 3,
    elevation: 2,
  },
  md: {
    shadowColor: '#101828',
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.08,
    shadowRadius: 16,
    elevation: 4,
  },
  lg: {
    shadowColor: '#101828',
    shadowOffset: { width: 0, height: 16 },
    shadowOpacity: 0.16,
    shadowRadius: 48,
    elevation: 10,
  },
} as const;

export const spacing = {
  xs: 4,
  sm: 8,
  md: 12,
  lg: 16,
  xl: 20,
  xxl: 24,
};

export const fontFamily = {
  regular: 'PlusJakartaSans_400Regular',
  medium: 'PlusJakartaSans_500Medium',
  semibold: 'PlusJakartaSans_600SemiBold',
  bold: 'PlusJakartaSans_700Bold',
  extrabold: 'PlusJakartaSans_800ExtraBold',
};
