import { render, screen } from '@testing-library/react-native';
import React from 'react';

import { ThemeProvider } from '../../theme/ThemeProvider';
import { Badge } from './Badge';

describe('Badge', () => {
  it('renders the given label', async () => {
    await render(
      <ThemeProvider>
        <Badge label="LIVE" tone="success" />
      </ThemeProvider>
    );
    expect(screen.getByText('LIVE')).toBeTruthy();
  });
});
