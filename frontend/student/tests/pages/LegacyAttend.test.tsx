import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import LegacyAttend from '../../src/pages/LegacyAttend';
import * as useIsMobileModule from '../../src/hooks/useIsMobile';

vi.mock('../../src/hooks/useIsMobile', () => ({
  useIsMobile: vi.fn(),
  useMobileVerification: vi.fn(() => ({
    isMobile: true,
    isEmulation: false,
    inconsistencies: [],
    checking: false,
    metrics: null,
    recheck: vi.fn(),
  })),
}));

describe('LegacyAttend', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (globalThis.fetch as any) = vi.fn().mockImplementation((url: string) => {
      if (url.includes('/api/storage-info')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            provider: 'cloudinary'
          })
        });
      }
      if (url.includes('/session')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            valid: true,
            session: { 
              locationName: 'Room 101',
              expiresAt: new Date(Date.now() + 3600000).toISOString(),
              isActive: true 
            },
            clientIp: '127.0.0.1',
            isBypassed: false
          })
        });
      }
      if (url.includes('/captcha')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            captchaId: 'test-captcha',
            captchaSvg: '<svg></svg>'
          })
        });
      }
      return Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
    });

    // Mock geolocation
    Object.defineProperty(globalThis.navigator, 'geolocation', {
      value: {
        getCurrentPosition: vi.fn().mockImplementation((success) => 
          success({ coords: { latitude: 40.7128, longitude: -74.0060, accuracy: 10 } })
        )
      },
      configurable: true
    });

    vi.mocked(useIsMobileModule.useIsMobile).mockReturnValue(true);
    vi.mocked(useIsMobileModule.useMobileVerification).mockReturnValue({
      isMobile: true,
      isEmulation: false,
      inconsistencies: [],
      checking: false,
      metrics: {} as any,
      recheck: vi.fn(),
    });
  });

  const renderComponent = () => render(
    <MemoryRouter initialEntries={['/s/legacycode']} future={{ v7_startTransition: true, v7_relativeSplatPath: true }}>
      <Routes>
        <Route path="/s/:shortCode" element={<LegacyAttend />} />
      </Routes>
    </MemoryRouter>
  );

  it('should render form after loading', async () => {
    renderComponent();
    await waitFor(() => {
      expect(screen.getByText(/Confirm details/i)).toBeInTheDocument();
    });
  });

  it('should display error if session is inactive', async () => {
    (globalThis.fetch as any) = vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
      json: () => Promise.resolve({ message: 'Session not found' })
    });

    renderComponent();
    await waitFor(() => {
      expect(screen.getByText(/Session not found/i)).toBeInTheDocument();
    });
  });
});
