import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import StudentScan from '../../src/pages/StudentScan';

// Mock matchMedia
window.matchMedia = window.matchMedia || function() {
  return {
    matches: false,
    addListener: function() {},
    removeListener: function() {}
  };
};

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

// Helper: after session loads, click through the permission onboarding screen
// so that the roll-number step is visible.
async function acknowledgeOnboarding() {
  // Wait for the onboarding screen ("Before You Begin")
  await waitFor(() =>
    expect(screen.getByText(/Before You Begin/i)).toBeInTheDocument()
  );
  // Click the acknowledge button to advance to rollInput step
  fireEvent.click(screen.getByRole('button', { name: /I Understand/i }));
  // Wait for the roll-number input to appear
  await waitFor(() =>
    expect(screen.getByText(/Mark Attendance/i)).toBeInTheDocument()
  );
}

describe('StudentScan', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (globalThis.fetch as any) = vi.fn().mockImplementation((url: string) => {
      if (url.includes('/session')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            session: { 
              locationName: 'Room 101', 
              expiresAt: new Date(Date.now() + 3600000).toISOString(),
              isActive: true,
              totpEnabled: true
            }
          })
        });
      }
      if (url.includes('/info')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            session: { isActive: true, totpEnabled: true },
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
      if (url.includes('/verify-gatekeeper')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            valid: true,
            isNewDevice: false,
            alreadySubmitted: false
          })
        });
      }
      return Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
    });
    
    // Mock navigator.geolocation
    const mockGeolocation = {
      getCurrentPosition: vi.fn().mockImplementation((success) => 
        success({
          coords: {
            latitude: 40.7128,
            longitude: -74.0060,
            accuracy: 10
          }
        })
      )
    };
    Object.defineProperty(globalThis.navigator, 'geolocation', {
      value: mockGeolocation,
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
    <MemoryRouter initialEntries={['/attend/testcode']}>
      <Routes>
        <Route path="/attend/:shortCode" element={<StudentScan />} />
      </Routes>
    </MemoryRouter>
  );

  it('should render loading state initially, then show permission onboarding', async () => {
    renderComponent();
    // Loading spinner shown first
    expect(screen.getByText(/Loading session.../i)).toBeInTheDocument();
    // Then the onboarding screen appears
    await waitFor(() =>
      expect(screen.getByText(/Before You Begin/i)).toBeInTheDocument()
    );
  });

  it('should advance to roll-number input after acknowledging onboarding', async () => {
    renderComponent();
    await acknowledgeOnboarding();
    // Roll number input should now be visible
    expect(screen.getByPlaceholderText(/e.g. 21CS042/i)).toBeInTheDocument();
  });

  it('should transition to form and acquire location', async () => {
    Object.defineProperty(window, 'PublicKeyCredential', { value: undefined, configurable: true });
    renderComponent();
    await acknowledgeOnboarding();
    
    // Step 1: Input roll number
    fireEvent.change(screen.getByPlaceholderText(/e.g. 21CS042/i), { target: { value: '21CS042' } });
    
    // Step 2: Fallback click (WebAuthn not supported)
    fireEvent.click(screen.getByText(/Continue without biometric/i));

    await waitFor(() => {
      expect(screen.getByText(/Location acquired/i)).toBeInTheDocument();
    });
  });

  it('intern-monitoring session skips onboarding/GPS entirely and goes straight to a registration-only roll-number step', async () => {
    (globalThis.fetch as any) = vi.fn().mockImplementation((url: string) => {
      if (url.includes('/session')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            session: {
              locationName: null,
              expiresAt: new Date(Date.now() + 3600000).toISOString(),
              isActive: true,
              sessionKind: 'intern_monitoring',
            },
          }),
        });
      }
      return Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
    });

    renderComponent();

    // No "Before You Begin" onboarding screen (no GPS/camera consent needed) —
    // straight to the roll-number step with intern-specific copy.
    await waitFor(() =>
      expect(screen.getByText(/Register for Monitoring/i)).toBeInTheDocument()
    );
    expect(screen.queryByText(/Before You Begin/i)).not.toBeInTheDocument();
    expect(screen.getByPlaceholderText(/e.g. 21CS042/i)).toBeInTheDocument();
  });

  it('intern-monitoring session has no manual-verification fallback when biometrics are unsupported', async () => {
    (globalThis.fetch as any) = vi.fn().mockImplementation((url: string) => {
      if (url.includes('/session')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            session: {
              locationName: null,
              expiresAt: new Date(Date.now() + 3600000).toISOString(),
              isActive: true,
              sessionKind: 'intern_monitoring',
            },
          }),
        });
      }
      if (url.includes('/webauthn/status/')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ enrolled: false, suspended: false, alreadySubmitted: false }),
        });
      }
      return Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
    });
    Object.defineProperty(window, 'PublicKeyCredential', { value: undefined, configurable: true });

    renderComponent();
    await waitFor(() =>
      expect(screen.getByText(/Register for Monitoring/i)).toBeInTheDocument()
    );

    // No manual-verification fallback for intern sessions — unsupported
    // biometrics just tells the intern to use a passkey-capable device.
    expect(screen.queryByText(/Continue without biometric/i)).not.toBeInTheDocument();
  });

  it('a monitored ordinary session shown on desktop gets pairing instructions, not the generic "mobile required" screen', async () => {
    vi.mocked(useIsMobileModule.useMobileVerification).mockReturnValue({
      isMobile: false,
      isEmulation: false,
      inconsistencies: [],
      checking: false,
      metrics: {} as any,
      recheck: vi.fn(),
    });
    (globalThis.fetch as any) = vi.fn().mockImplementation((url: string) => {
      if (url.includes('/session')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            session: {
              locationName: 'Room 101',
              expiresAt: new Date(Date.now() + 3600000).toISOString(),
              isActive: true,
              sessionKind: 'attendance',
              monitoringEnabled: true,
            },
          }),
        });
      }
      return Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
    });

    renderComponent();

    // The desktop gate excludes the 'permissions' step (it shows onboarding
    // first regardless of device), so it only kicks in once the student
    // clicks through it — same as real behavior.
    await waitFor(() =>
      expect(screen.getByText(/Before You Begin/i)).toBeInTheDocument()
    );
    fireEvent.click(screen.getByRole('button', { name: /I Understand/i }));

    await waitFor(() =>
      expect(screen.getByText(/Keep This Page/i)).toBeInTheDocument()
    );
    expect(screen.getByText(/Monitoring is on for this session/i)).toBeInTheDocument();
    // Neither the generic desktop-blocked copy nor the intern-only copy.
    expect(screen.queryByText(/For security and GPS verification/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/register your passkey for the first time/i)).not.toBeInTheDocument();
  });

  it('an ordinary, unmonitored session shown on desktop still gets the generic "mobile required" screen', async () => {
    vi.mocked(useIsMobileModule.useMobileVerification).mockReturnValue({
      isMobile: false,
      isEmulation: false,
      inconsistencies: [],
      checking: false,
      metrics: {} as any,
      recheck: vi.fn(),
    });
    (globalThis.fetch as any) = vi.fn().mockImplementation((url: string) => {
      if (url.includes('/session')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            session: {
              locationName: 'Room 101',
              expiresAt: new Date(Date.now() + 3600000).toISOString(),
              isActive: true,
              sessionKind: 'attendance',
              monitoringEnabled: false,
            },
          }),
        });
      }
      return Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
    });

    renderComponent();

    await waitFor(() =>
      expect(screen.getByText(/Before You Begin/i)).toBeInTheDocument()
    );
    fireEvent.click(screen.getByRole('button', { name: /I Understand/i }));

    await waitFor(() =>
      expect(screen.getByText(/Mobile Access/i)).toBeInTheDocument()
    );
    expect(screen.queryByText(/Monitoring is on for this session/i)).not.toBeInTheDocument();
  });

  it('should handle location denial', async () => {
    const mockGeolocation = {
      getCurrentPosition: vi.fn().mockImplementation((_, error) => 
        error({ code: 1, message: 'User denied geolocation' })
      )
    };
    Object.defineProperty(globalThis.navigator, 'geolocation', {
      value: mockGeolocation,
      configurable: true
    });

    Object.defineProperty(window, 'PublicKeyCredential', { value: undefined, configurable: true });
    renderComponent();
    await acknowledgeOnboarding();
    
    fireEvent.change(screen.getByPlaceholderText(/e.g. 21CS042/i), { target: { value: '21CS042' } });
    fireEvent.click(screen.getByText(/Continue without biometric/i));

    await waitFor(() => {
      expect(screen.getByText(/Location permission blocked/i)).toBeInTheDocument();
    });
  });
});
