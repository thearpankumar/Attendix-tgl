import { TriangleAlert } from 'lucide-react-native';
import React, { Component, ErrorInfo, ReactNode } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { lightColors } from '../theme/tokens';
import { Button } from './ui/Button';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
}

// Root-level safety net for render crashes. Deliberately theme-independent
// (reads tokens directly rather than useTheme()) since it can render above
// ThemeProvider in the tree if that itself ever throws.
export class ErrorBoundary extends Component<Props, State> {
  public state: State = { hasError: false };

  public static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    if (__DEV__) {
      console.error('Uncaught error:', error, errorInfo);
    }
  }

  private reset = () => this.setState({ hasError: false });

  public render() {
    if (this.state.hasError) {
      const colors = lightColors;
      return (
        <View style={[styles.screen, { backgroundColor: colors.bg }]}>
          <View style={[styles.card, { backgroundColor: colors.surface, borderColor: colors.border }]}>
            <View style={[styles.iconWrap, { backgroundColor: colors.dangerBg }]}>
              <TriangleAlert size={28} color={colors.danger} />
            </View>
            <Text style={[styles.title, { color: colors.text }]}>Something went wrong</Text>
            <Text style={[styles.subtitle, { color: colors.muted }]}>An unexpected error occurred.</Text>
            <Button title="Try Again" onPress={this.reset} block />
          </View>
        </View>
      );
    }

    return this.props.children;
  }
}

const styles = StyleSheet.create({
  screen: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 24 },
  card: { width: '100%', maxWidth: 380, borderWidth: 1, borderRadius: 20, padding: 32, alignItems: 'center', gap: 8 },
  iconWrap: { width: 64, height: 64, borderRadius: 32, alignItems: 'center', justifyContent: 'center', marginBottom: 8 },
  title: { fontSize: 19, fontWeight: '800' },
  subtitle: { fontSize: 14, textAlign: 'center', marginBottom: 12 },
});
