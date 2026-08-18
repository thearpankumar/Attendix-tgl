import React, { useEffect, useState } from 'react';
import { Modal, Text, View, StyleSheet } from 'react-native';

import { useAppUpdate } from '../hooks/useAppUpdate';
import { useTheme } from '../theme/ThemeProvider';
import { UpdateDetails } from './UpdateDetails';
import { Button } from './ui/Button';

// Mounted once at the (app) group root so it covers both a fresh login and a
// cold start with an already-cached session. Checks once per mount; if the
// user taps "Later" it just hides for this app session — there's no
// persisted skip/snooze, so it reappears on the next cold start if the
// device is still behind.
export const UpdatePrompt = () => {
  const { colors, radii, font } = useTheme();
  const update = useAppUpdate();
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    update.check();
    // Only ever check once per mount of this component (app cold start /
    // fresh login) — not on every re-render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const visible = update.updateAvailable && !dismissed;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={() => {}}>
      <View style={styles.backdrop}>
        <View style={[styles.card, { backgroundColor: colors.surface, borderRadius: radii.lg }]}>
          <Text style={[styles.title, { color: colors.text, fontFamily: font.extrabold }]}>Update available</Text>

          {update.latestVersion ? (
            <UpdateDetails
              currentVersion={update.currentVersion}
              latestVersion={update.latestVersion}
              notes={update.notes}
              downloading={update.downloading}
              progress={update.progress}
              error={update.error}
              onUpdate={update.download}
            />
          ) : null}

          {!update.downloading ? (
            <View style={{ marginTop: 12 }}>
              <Button title="Later" variant="secondary" onPress={() => setDismissed(true)} block />
            </View>
          ) : null}
        </View>
      </View>
    </Modal>
  );
};

const styles = StyleSheet.create({
  backdrop: {
    flex: 1,
    backgroundColor: 'rgba(0,0,0,0.5)',
    alignItems: 'center',
    justifyContent: 'center',
    padding: 24,
  },
  card: {
    width: '100%',
    maxWidth: 400,
    padding: 20,
  },
  title: { fontSize: 18, marginBottom: 12 },
});
