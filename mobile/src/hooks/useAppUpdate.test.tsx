/* eslint-disable import/first -- jest.mock must precede the imports it mocks */
jest.mock('expo-constants', () => ({
  __esModule: true,
  default: { expoConfig: { version: '1.1.4' } },
}));

jest.mock('../api/githubRelease', () => ({
  fetchLatestMobileRelease: jest.fn(),
}));

const mockCreateDownloadResumable = jest.fn();
const mockGetContentUriAsync = jest.fn();
jest.mock('expo-file-system/legacy', () => ({
  cacheDirectory: 'file:///cache/',
  createDownloadResumable: (...args: unknown[]) => mockCreateDownloadResumable(...args),
  getContentUriAsync: (...args: unknown[]) => mockGetContentUriAsync(...args),
}));

const mockStartActivityAsync = jest.fn();
jest.mock('expo-intent-launcher', () => ({
  startActivityAsync: (...args: unknown[]) => mockStartActivityAsync(...args),
}));

import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react-native';
import React from 'react';
import { Platform } from 'react-native';

import { fetchLatestMobileRelease } from '../api/githubRelease';
import { useAppUpdate } from './useAppUpdate';

const mockedFetch = fetchLatestMobileRelease as jest.Mock;

// A fresh QueryClient per test, captured in a stable wrapper component
// reference — recreating the client on every render (e.g. by constructing
// it inline in the wrapper body) would reset the query cache on every
// re-render triggered by the hook's own state changes.
//
// gcTime: 0 is load-bearing, not just tidy defaults: react-query's default
// gcTime (5 minutes) schedules a real setTimeout per query to garbage
// collect it, which outlives the test and is exactly what trips Jest's
// "did not exit one second after the test run" warning — queryClient.clear()
// / .unmount() do NOT reliably cancel that already-scheduled timer (see
// https://github.com/TanStack/query/issues/1847). gcTime: 0 collects
// synchronously instead of scheduling a timer at all.
const activeClients: QueryClient[] = [];
function createWrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  activeClients.push(client);
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

describe('useAppUpdate', () => {
  const originalOS = Platform.OS;

  beforeEach(() => {
    jest.clearAllMocks();
    Object.defineProperty(Platform, 'OS', { get: () => 'android' });
  });

  afterEach(() => {
    Object.defineProperty(Platform, 'OS', { get: () => originalOS });
    activeClients.forEach((client) => {
      client.clear();
      client.unmount();
    });
    activeClients.length = 0;
  });

  it('reports no update available before check() has run', async () => {
    const { result } = await renderHook(() => useAppUpdate(), { wrapper: createWrapper() });

    expect(result.current.updateAvailable).toBe(false);
    expect(result.current.hasChecked).toBe(false);
    expect(result.current.currentVersion).toBe('1.1.4');
  });

  it('reports an update available when the release is newer than the installed version', async () => {
    mockedFetch.mockResolvedValue({ version: '1.2.0', downloadUrl: 'https://example.com/app.apk', notes: 'New stuff' });
    const { result } = await renderHook(() => useAppUpdate(), { wrapper: createWrapper() });

    await act(async () => {
      await result.current.check();
    });
    await waitFor(() => expect(result.current.hasChecked).toBe(true));

    expect(result.current.updateAvailable).toBe(true);
    expect(result.current.latestVersion).toBe('1.2.0');
    expect(result.current.notes).toBe('New stuff');
  });

  it('reports no update available when the release is the same as the installed version', async () => {
    mockedFetch.mockResolvedValue({ version: '1.1.4', downloadUrl: 'https://example.com/app.apk', notes: '' });
    const { result } = await renderHook(() => useAppUpdate(), { wrapper: createWrapper() });

    await act(async () => {
      await result.current.check();
    });
    await waitFor(() => expect(result.current.hasChecked).toBe(true));

    expect(result.current.updateAvailable).toBe(false);
  });

  it('reports no update available when there is no parseable mobile release', async () => {
    mockedFetch.mockResolvedValue(null);
    const { result } = await renderHook(() => useAppUpdate(), { wrapper: createWrapper() });

    await act(async () => {
      await result.current.check();
    });
    await waitFor(() => expect(result.current.hasChecked).toBe(true));

    expect(result.current.updateAvailable).toBe(false);
    expect(result.current.latestVersion).toBeNull();
  });

  it('surfaces a check failure without throwing', async () => {
    mockedFetch.mockRejectedValue(new Error('network down'));
    const { result } = await renderHook(() => useAppUpdate(), { wrapper: createWrapper() });

    await act(async () => {
      await result.current.check();
    });
    await waitFor(() => expect(result.current.error).toBe('Could not check for updates'));

    expect(result.current.updateAvailable).toBe(false);
  });

  describe('download', () => {
    async function primeAvailableUpdate() {
      mockedFetch.mockResolvedValue({ version: '1.2.0', downloadUrl: 'https://example.com/app.apk', notes: '' });
      const { result } = await renderHook(() => useAppUpdate(), { wrapper: createWrapper() });
      await act(async () => {
        await result.current.check();
      });
      await waitFor(() => expect(result.current.updateAvailable).toBe(true));
      return result;
    }

    it('downloads the APK, converts it to a content URI, and fires the install intent', async () => {
      const result = await primeAvailableUpdate();

      mockCreateDownloadResumable.mockImplementation((_uri, fileUri, _options, onProgress) => ({
        downloadAsync: jest.fn(async () => {
          onProgress({ totalBytesWritten: 50, totalBytesExpectedToWrite: 100 });
          return { uri: fileUri };
        }),
      }));
      mockGetContentUriAsync.mockResolvedValue('content://com.arpankumar1119.attendixmentor.fileprovider/apk');
      mockStartActivityAsync.mockResolvedValue(undefined);

      await act(async () => {
        await result.current.download();
      });

      expect(mockCreateDownloadResumable).toHaveBeenCalledWith(
        'https://example.com/app.apk',
        'file:///cache/attendix-mentor-update.apk',
        {},
        expect.any(Function)
      );
      expect(mockGetContentUriAsync).toHaveBeenCalledWith('file:///cache/attendix-mentor-update.apk');
      expect(mockStartActivityAsync).toHaveBeenCalledWith('android.intent.action.INSTALL_PACKAGE', {
        data: 'content://com.arpankumar1119.attendixmentor.fileprovider/apk',
        flags: 1,
        type: 'application/vnd.android.package-archive',
      });
      expect(result.current.downloading).toBe(false);
      expect(result.current.progress).toBe(0.5);
      expect(result.current.error).toBeNull();
    });

    it('sets an error and stops downloading if the download fails', async () => {
      const result = await primeAvailableUpdate();

      mockCreateDownloadResumable.mockImplementation(() => ({
        downloadAsync: jest.fn().mockRejectedValue(new Error('disk full')),
      }));

      await act(async () => {
        await result.current.download();
      });

      expect(result.current.downloading).toBe(false);
      expect(result.current.error).toBe('disk full');
      expect(mockStartActivityAsync).not.toHaveBeenCalled();
    });

    it('does nothing when there is no release to download yet', async () => {
      const { result } = await renderHook(() => useAppUpdate(), { wrapper: createWrapper() });

      await act(async () => {
        await result.current.download();
      });

      expect(mockCreateDownloadResumable).not.toHaveBeenCalled();
    });

    it('does nothing on non-Android platforms', async () => {
      const result = await primeAvailableUpdate();
      Object.defineProperty(Platform, 'OS', { get: () => 'ios' });

      await act(async () => {
        await result.current.download();
      });

      expect(mockCreateDownloadResumable).not.toHaveBeenCalled();
    });
  });
});
