/* eslint-disable import/first -- jest.mock must precede the imports it mocks */
jest.mock('../hooks/useAppUpdate', () => ({
  useAppUpdate: jest.fn(),
}));

import { fireEvent, render, screen } from '@testing-library/react-native';
import React from 'react';

import { useAppUpdate } from '../hooks/useAppUpdate';
import { ThemeProvider } from '../theme/ThemeProvider';
import { UpdatePrompt } from './UpdatePrompt';

const mockedUseAppUpdate = useAppUpdate as jest.Mock;

const baseState = {
  checking: false,
  hasChecked: true,
  updateAvailable: false,
  latestVersion: null as string | null,
  currentVersion: '1.1.4',
  notes: '',
  check: jest.fn(),
  downloading: false,
  progress: 0,
  download: jest.fn(),
  error: null as string | null,
};

const renderPrompt = () => render(<ThemeProvider><UpdatePrompt /></ThemeProvider>);

describe('UpdatePrompt', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders nothing when no update is available', async () => {
    mockedUseAppUpdate.mockReturnValue({ ...baseState, updateAvailable: false });
    await renderPrompt();

    expect(screen.queryByText('Update available')).toBeNull();
  });

  it('calls check() exactly once on mount', async () => {
    const check = jest.fn();
    mockedUseAppUpdate.mockReturnValue({ ...baseState, check });
    await renderPrompt();

    expect(check).toHaveBeenCalledTimes(1);
  });

  it('shows the update modal with version details when an update is available', async () => {
    mockedUseAppUpdate.mockReturnValue({
      ...baseState,
      updateAvailable: true,
      latestVersion: '1.2.0',
      notes: 'Bug fixes',
    });
    await renderPrompt();

    expect(screen.getByText('Update available')).toBeTruthy();
    expect(screen.getByText(/1\.2\.0/)).toBeTruthy();
    expect(screen.getByText('Bug fixes')).toBeTruthy();
  });

  it('dismisses the modal for this session when "Later" is tapped', async () => {
    mockedUseAppUpdate.mockReturnValue({
      ...baseState,
      updateAvailable: true,
      latestVersion: '1.2.0',
    });
    await renderPrompt();

    expect(screen.getByText('Update available')).toBeTruthy();
    // Must be awaited: with React 19, the resulting setState/re-render
    // (the modal returning null) doesn't flush before the next assertion
    // otherwise — the query below would still see the pre-press tree.
    await fireEvent.press(screen.getByText('Later'));

    expect(screen.queryByText('Update available')).toBeNull();
  });

  it('calls download() when "Update Now" is tapped', async () => {
    const download = jest.fn();
    mockedUseAppUpdate.mockReturnValue({
      ...baseState,
      updateAvailable: true,
      latestVersion: '1.2.0',
      download,
    });
    await renderPrompt();

    await fireEvent.press(screen.getByText('Update Now'));
    expect(download).toHaveBeenCalledTimes(1);
  });

  it('hides the "Later" button while a download is in progress', async () => {
    mockedUseAppUpdate.mockReturnValue({
      ...baseState,
      updateAvailable: true,
      latestVersion: '1.2.0',
      downloading: true,
      progress: 0.4,
    });
    await renderPrompt();

    expect(screen.queryByText('Later')).toBeNull();
    expect(screen.getByText(/40%/)).toBeTruthy();
  });
});
