import { fireEvent, render, screen } from '@testing-library/react-native';
import React from 'react';

import { ThemeProvider } from '../../theme/ThemeProvider';
import { Button } from './Button';

const renderWithTheme = (ui: React.ReactElement) => render(<ThemeProvider>{ui}</ThemeProvider>);

describe('Button', () => {
  it('renders its title', async () => {
    await renderWithTheme(<Button title="Sign In" onPress={jest.fn()} />);
    expect(screen.getByText('Sign In')).toBeTruthy();
  });

  it('calls onPress when tapped', async () => {
    const onPress = jest.fn();
    await renderWithTheme(<Button title="Sign In" onPress={onPress} />);

    fireEvent.press(screen.getByText('Sign In'));
    expect(onPress).toHaveBeenCalledTimes(1);
  });

  it('does not call onPress while loading', async () => {
    const onPress = jest.fn();
    await renderWithTheme(<Button title="Sign In" onPress={onPress} loading />);

    fireEvent.press(screen.getByText('Sign In'));
    expect(onPress).not.toHaveBeenCalled();
  });

  it('does not call onPress while disabled', async () => {
    const onPress = jest.fn();
    await renderWithTheme(<Button title="Sign In" onPress={onPress} disabled />);

    fireEvent.press(screen.getByText('Sign In'));
    expect(onPress).not.toHaveBeenCalled();
  });
});
