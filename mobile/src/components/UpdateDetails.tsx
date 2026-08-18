import { Download } from 'lucide-react-native';
import React from 'react';
import { Text, View, StyleSheet } from 'react-native';

import { useTheme } from '../theme/ThemeProvider';
import { Button } from './ui/Button';
import { ProgressBar } from './ui/ProgressBar';

interface UpdateDetailsProps {
  currentVersion: string;
  latestVersion: string;
  notes: string;
  downloading: boolean;
  progress: number;
  error: string | null;
  onUpdate: () => void;
}

// Shared body used both inside the startup popup (UpdatePrompt, wrapped in a
// Modal) and inline in the Settings "App Update" card — keeps the
// download/progress-bar rendering in one place instead of duplicating it.
export const UpdateDetails = ({
  currentVersion,
  latestVersion,
  notes,
  downloading,
  progress,
  error,
  onUpdate,
}: UpdateDetailsProps) => {
  const { colors, font } = useTheme();

  return (
    <View>
      <Text style={{ color: colors.muted, fontSize: 13, marginBottom: 12 }}>
        Version <Text style={{ fontFamily: font.bold, color: colors.text }}>{latestVersion}</Text> is available
        (you have {currentVersion}).
      </Text>

      {notes ? (
        <Text style={{ color: colors.muted, fontSize: 12, marginBottom: 16 }} numberOfLines={4}>
          {notes}
        </Text>
      ) : null}

      {downloading ? (
        <View style={{ marginBottom: 4 }}>
          <ProgressBar percent={progress * 100} />
          <Text style={{ color: colors.muted, fontSize: 12, marginTop: 8 }}>
            Downloading update… {Math.round(progress * 100)}%
          </Text>
        </View>
      ) : (
        <Button
          title={error ? 'Retry Update' : 'Update Now'}
          onPress={onUpdate}
          icon={<Download size={16} color={colors.onPrimary} />}
          block
        />
      )}

      {error ? (
        <Text style={[styles.error, { color: colors.dangerTxt }]}>{error}</Text>
      ) : null}
    </View>
  );
};

const styles = StyleSheet.create({
  error: { fontSize: 12, marginTop: 8 },
});
