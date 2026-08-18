import { fireEvent, render, screen } from '@testing-library/react-native';
import React from 'react';

import { ThemeProvider } from '../theme/ThemeProvider';
import { UpdateDetails } from './UpdateDetails';

const baseProps = {
  currentVersion: '1.1.4',
  latestVersion: '1.2.0',
  notes: '',
  downloading: false,
  progress: 0,
  error: null as string | null,
  onUpdate: jest.fn(),
};

const renderDetails = (props: Partial<typeof baseProps> = {}) =>
  render(<ThemeProvider><UpdateDetails {...baseProps} {...props} /></ThemeProvider>);

describe('UpdateDetails', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('shows the current and latest version', async () => {
    await renderDetails();

    expect(screen.getByText(/1\.2\.0/)).toBeTruthy();
    expect(screen.getByText(/1\.1\.4/)).toBeTruthy();
  });

  it('shows release notes when present, and omits them when empty', async () => {
    const { rerender } = await renderDetails({ notes: 'Fixed the login bug' });
    expect(screen.getByText('Fixed the login bug')).toBeTruthy();

    await rerender(<ThemeProvider><UpdateDetails {...baseProps} notes="" /></ThemeProvider>);
    expect(screen.queryByText('Fixed the login bug')).toBeNull();
  });

  it('calls onUpdate when "Update Now" is tapped', async () => {
    const onUpdate = jest.fn();
    await renderDetails({ onUpdate });

    await fireEvent.press(screen.getByText('Update Now'));
    expect(onUpdate).toHaveBeenCalledTimes(1);
  });

  it('shows a progress bar and percentage while downloading, and hides the button', async () => {
    await renderDetails({ downloading: true, progress: 0.42 });

    expect(screen.getByText(/42%/)).toBeTruthy();
    expect(screen.queryByText('Update Now')).toBeNull();
  });

  it('labels the button "Retry Update" and shows the error message after a failed download', async () => {
    await renderDetails({ error: 'Failed to download the update' });

    expect(screen.getByText('Retry Update')).toBeTruthy();
    expect(screen.getByText('Failed to download the update')).toBeTruthy();
  });
});
