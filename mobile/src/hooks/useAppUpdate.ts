import { useQuery } from '@tanstack/react-query';
import Constants from 'expo-constants';
// SDK 57's new expo-file-system API doesn't expose getContentUriAsync yet
// (https://github.com/expo/expo/issues/39056) — it only exists on the
// legacy entry point, which is still what we need to hand the downloaded
// APK's content:// URI to the system installer.
import * as FileSystem from 'expo-file-system/legacy';
import * as IntentLauncher from 'expo-intent-launcher';
import { useCallback, useState } from 'react';
import { Platform } from 'react-native';

import { fetchLatestMobileRelease } from '../api/githubRelease';
import { isNewerVersion } from '../utils/version';

const APK_MIME_TYPE = 'application/vnd.android.package-archive';
// Intent.FLAG_GRANT_READ_URI_PERMISSION — lets the installer app read our
// FileProvider content:// URI without a matching manifest permission.
const FLAG_GRANT_READ_URI_PERMISSION = 1;

export function useAppUpdate() {
  const currentVersion = (Constants.expoConfig?.version as string | undefined) ?? '0.0.0';
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState(0);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  // Shared queryKey with any other useAppUpdate() consumer (the startup
  // prompt and the Settings screen both use this hook) — react-query dedupes
  // the cache and fetch state across them, so checking once surfaces
  // everywhere without a second network round-trip.
  const query = useQuery({
    queryKey: ['app-update-check'],
    queryFn: fetchLatestMobileRelease,
    enabled: false,
    retry: false,
    staleTime: 0,
  });

  const release = query.data ?? null;
  const updateAvailable = !!release && isNewerVersion(release.version, currentVersion);

  const check = useCallback(() => {
    setDownloadError(null);
    return query.refetch();
  }, [query]);

  const download = useCallback(async () => {
    if (!release || Platform.OS !== 'android') return;
    setDownloading(true);
    setProgress(0);
    setDownloadError(null);
    try {
      const fileUri = `${FileSystem.cacheDirectory}attendix-mentor-update.apk`;
      const downloadResumable = FileSystem.createDownloadResumable(
        release.downloadUrl,
        fileUri,
        {},
        (data) => {
          if (data.totalBytesExpectedToWrite > 0) {
            setProgress(data.totalBytesWritten / data.totalBytesExpectedToWrite);
          }
        }
      );
      const result = await downloadResumable.downloadAsync();
      if (!result) throw new Error('Download did not complete');

      const contentUri = await FileSystem.getContentUriAsync(result.uri);
      await IntentLauncher.startActivityAsync('android.intent.action.INSTALL_PACKAGE', {
        data: contentUri,
        flags: FLAG_GRANT_READ_URI_PERMISSION,
        type: APK_MIME_TYPE,
      });
    } catch (error) {
      setDownloadError(error instanceof Error ? error.message : 'Failed to download the update');
    } finally {
      setDownloading(false);
    }
  }, [release]);

  return {
    checking: query.isFetching,
    hasChecked: query.isFetched,
    updateAvailable,
    latestVersion: release?.version ?? null,
    currentVersion,
    notes: release?.notes ?? '',
    check,
    downloading,
    progress,
    download,
    error: downloadError || (query.isError ? 'Could not check for updates' : null),
  };
}
