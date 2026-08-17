import {
  PlusJakartaSans_400Regular,
  PlusJakartaSans_500Medium,
  PlusJakartaSans_600SemiBold,
  PlusJakartaSans_700Bold,
  PlusJakartaSans_800ExtraBold,
  useFonts,
} from '@expo-google-fonts/plus-jakarta-sans';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Stack } from 'expo-router';
import * as SplashScreen from 'expo-splash-screen';
import React, { useEffect, useRef } from 'react';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import Toast from 'react-native-toast-message';

import { ErrorBoundary } from '../components/ErrorBoundary';
import { AuthProvider, useAuth } from '../context/AuthContext';
import { BiometricLockProvider, useBiometricLock } from '../context/BiometricLockContext';
import { ThemeProvider } from '../theme/ThemeProvider';

SplashScreen.preventAutoHideAsync();

const queryClient = new QueryClient();

// Resets the biometric-lock state whenever a session ends, whether by an
// explicit logout or an automatic 401 (expired/blacklisted token) — either
// way there's nothing left to unlock, and the next login should re-offer
// biometric enrollment from scratch.
function SessionWatcher() {
  const { admin } = useAuth();
  const { resetLock } = useBiometricLock();
  const prevAdmin = useRef(admin);

  useEffect(() => {
    if (prevAdmin.current && !admin) {
      resetLock();
    }
    prevAdmin.current = admin;
  }, [admin, resetLock]);

  return null;
}

function SplashGate({ fontsLoaded }: { fontsLoaded: boolean }) {
  const { loading } = useAuth();
  const { bootstrapped } = useBiometricLock();

  useEffect(() => {
    if (fontsLoaded && !loading && bootstrapped) {
      SplashScreen.hideAsync();
    }
  }, [fontsLoaded, loading, bootstrapped]);

  return null;
}

function RootNavigator({ fontsLoaded }: { fontsLoaded: boolean }) {
  const { admin, loading } = useAuth();
  const { locked, bootstrapped } = useBiometricLock();

  if (!fontsLoaded || loading || !bootstrapped) return null;

  return (
    <Stack screenOptions={{ headerShown: false }}>
      {/* (app) declared first so it's the fallback screen when this group
          becomes active with no resolvable current route (e.g. right after
          a biometric unlock from the completely separate `locked` group) —
          expo-router's Stack.Protected falls back to "the anchor route...
          or the first available screen in the stack" in that case, and an
          already-enrolled user unlocking successfully must land on the
          dashboard, not back on the enrollment screen. */}
      <Stack.Protected guard={!!admin && !locked}>
        <Stack.Screen name="(app)" />
        <Stack.Screen name="enroll-biometric" />
      </Stack.Protected>
      <Stack.Protected guard={!!admin && locked}>
        <Stack.Screen name="unlock" />
      </Stack.Protected>
      <Stack.Protected guard={!admin}>
        <Stack.Screen name="login" />
      </Stack.Protected>
    </Stack>
  );
}

export default function RootLayout() {
  const [fontsLoaded] = useFonts({
    PlusJakartaSans_400Regular,
    PlusJakartaSans_500Medium,
    PlusJakartaSans_600SemiBold,
    PlusJakartaSans_700Bold,
    PlusJakartaSans_800ExtraBold,
  });

  return (
    <ErrorBoundary>
      <GestureHandlerRootView style={{ flex: 1 }}>
        <SafeAreaProvider>
          <ThemeProvider>
            <QueryClientProvider client={queryClient}>
              <AuthProvider>
                <BiometricLockProvider>
                  <SessionWatcher />
                  <SplashGate fontsLoaded={fontsLoaded} />
                  <RootNavigator fontsLoaded={fontsLoaded} />
                  <Toast />
                </BiometricLockProvider>
              </AuthProvider>
            </QueryClientProvider>
          </ThemeProvider>
        </SafeAreaProvider>
      </GestureHandlerRootView>
    </ErrorBoundary>
  );
}
