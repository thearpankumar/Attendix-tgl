/* eslint-disable import/first -- jest.mock must precede the imports it mocks */
jest.mock('expo-router', () => ({
  useRouter: () => ({ push: jest.fn(), back: jest.fn() }),
}));

jest.mock('react-native-safe-area-context', () => ({
  useSafeAreaInsets: () => ({ top: 0, right: 0, bottom: 0, left: 0 }),
}));

jest.mock('../../context/AuthContext', () => ({
  useAuth: jest.fn(),
}));

jest.mock('../../context/BiometricLockContext', () => ({
  useBiometricLock: jest.fn(),
}));

jest.mock('../../hooks/useAppUpdate', () => ({
  useAppUpdate: jest.fn(),
}));

import { fireEvent, render, screen } from '@testing-library/react-native';
import React from 'react';

import { Admin } from '../../api/types';
import { useAuth } from '../../context/AuthContext';
import { useBiometricLock } from '../../context/BiometricLockContext';
import { useAppUpdate } from '../../hooks/useAppUpdate';
import { ThemeProvider } from '../../theme/ThemeProvider';
import Settings from './settings';

const mockedUseAuth = useAuth as jest.Mock;
const mockedUseBiometricLock = useBiometricLock as jest.Mock;
const mockedUseAppUpdate = useAppUpdate as jest.Mock;

const admin: Admin = { _id: '1', username: 'mentor1', email: 'm@example.com', role: 'admin', fullName: 'Mentor One' };

const renderSettings = () => render(<ThemeProvider><Settings /></ThemeProvider>);

describe('Settings — Change Password collapsible card', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockedUseAuth.mockReturnValue({ admin, logout: jest.fn(), changePassword: jest.fn() });
    mockedUseBiometricLock.mockReturnValue({
      capability: { hasHardware: false, isEnrolled: false, types: [], label: 'Biometric unlock' },
      enabled: false,
      enroll: jest.fn(),
      disable: jest.fn(),
    });
    mockedUseAppUpdate.mockReturnValue({
      checking: false,
      hasChecked: false,
      updateAvailable: false,
      latestVersion: null,
      currentVersion: '1.1.4',
      notes: '',
      check: jest.fn(),
      downloading: false,
      progress: 0,
      download: jest.fn(),
      error: null,
    });
  });

  it('starts collapsed: the password fields are not on screen', async () => {
    await renderSettings();

    expect(screen.getByText('Change Password')).toBeTruthy();
    expect(screen.queryByText('Current Password')).toBeNull();
    expect(screen.queryByText('New Password')).toBeNull();
    expect(screen.queryByText('Confirm New Password')).toBeNull();
    expect(screen.queryByText('Update Password')).toBeNull();
  });

  it('reveals the password fields when the header is tapped', async () => {
    await renderSettings();

    await fireEvent.press(screen.getByText('Change Password'));

    expect(screen.getByText('Current Password')).toBeTruthy();
    expect(screen.getByText('New Password')).toBeTruthy();
    expect(screen.getByText('Confirm New Password')).toBeTruthy();
    expect(screen.getByText('Update Password')).toBeTruthy();
  });

  it('collapses the fields again on a second tap', async () => {
    await renderSettings();

    await fireEvent.press(screen.getByText('Change Password'));
    expect(screen.getByText('Current Password')).toBeTruthy();

    await fireEvent.press(screen.getByText('Change Password'));
    expect(screen.queryByText('Current Password')).toBeNull();
  });
});

describe('Settings — App Update card', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockedUseAuth.mockReturnValue({ admin, logout: jest.fn(), changePassword: jest.fn() });
    mockedUseBiometricLock.mockReturnValue({
      capability: { hasHardware: false, isEnrolled: false, types: [], label: 'Biometric unlock' },
      enabled: false,
      enroll: jest.fn(),
      disable: jest.fn(),
    });
  });

  it('shows the current version and a "Check for Updates" button before any check has run', async () => {
    mockedUseAppUpdate.mockReturnValue({
      checking: false,
      hasChecked: false,
      updateAvailable: false,
      latestVersion: null,
      currentVersion: '1.1.4',
      notes: '',
      check: jest.fn(),
      downloading: false,
      progress: 0,
      download: jest.fn(),
      error: null,
    });
    await renderSettings();

    expect(screen.getByText(/1\.1\.4/)).toBeTruthy();
    expect(screen.getByText('Check for Updates')).toBeTruthy();
    expect(screen.queryByText("You're up to date.")).toBeNull();
  });

  it('shows "up to date" after a check finds no newer release', async () => {
    mockedUseAppUpdate.mockReturnValue({
      checking: false,
      hasChecked: true,
      updateAvailable: false,
      latestVersion: null,
      currentVersion: '1.1.4',
      notes: '',
      check: jest.fn(),
      downloading: false,
      progress: 0,
      download: jest.fn(),
      error: null,
    });
    await renderSettings();

    expect(screen.getByText("You're up to date.")).toBeTruthy();
  });

  it('shows the update details inline (no "Check for Updates" button) when an update is available', async () => {
    mockedUseAppUpdate.mockReturnValue({
      checking: false,
      hasChecked: true,
      updateAvailable: true,
      latestVersion: '1.2.0',
      currentVersion: '1.1.4',
      notes: '',
      check: jest.fn(),
      downloading: false,
      progress: 0,
      download: jest.fn(),
      error: null,
    });
    await renderSettings();

    expect(screen.getByText('Update Now')).toBeTruthy();
    expect(screen.queryByText('Check for Updates')).toBeNull();
  });

  it('calls check() when "Check for Updates" is tapped', async () => {
    const check = jest.fn();
    mockedUseAppUpdate.mockReturnValue({
      checking: false,
      hasChecked: false,
      updateAvailable: false,
      latestVersion: null,
      currentVersion: '1.1.4',
      notes: '',
      check,
      downloading: false,
      progress: 0,
      download: jest.fn(),
      error: null,
    });
    await renderSettings();

    await fireEvent.press(screen.getByText('Check for Updates'));
    expect(check).toHaveBeenCalledTimes(1);
  });
});
