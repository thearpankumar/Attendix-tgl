import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
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
  });

  const renderComponent = () => render(
    <MemoryRouter initialEntries={['/attend/testcode']} future={{ v7_startTransition: true, v7_relativeSplatPath: true }}>
      <Routes>
        <Route path="/attend/:shortCode" element={<StudentScan />} />
      </Routes>
    </MemoryRouter>
  );

  it('should render loading state initially', async () => {
    renderComponent();
    expect(screen.getByText(/Loading session.../i)).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText(/Mark Attendance/i)).toBeInTheDocument());
  });

  it('should transition to form and acquire location', async () => {
    Object.defineProperty(window, 'PublicKeyCredential', { value: undefined, configurable: true });
    renderComponent();
    await waitFor(() => expect(screen.getByText(/Mark Attendance/i)).toBeInTheDocument());
    
    // Step 1: Input roll number
    fireEvent.change(screen.getByPlaceholderText(/e.g. 21CS042/i), { target: { value: '21CS042' } });
    
    // Step 2: Fallback click
    fireEvent.click(screen.getByText(/Continue without biometric/i));

    await waitFor(() => {
      expect(screen.getByText(/Location acquired/i)).toBeInTheDocument();
    });
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
    await waitFor(() => expect(screen.getByText(/Mark Attendance/i)).toBeInTheDocument());
    
    fireEvent.change(screen.getByPlaceholderText(/e.g. 21CS042/i), { target: { value: '21CS042' } });
    fireEvent.click(screen.getByText(/Continue without biometric/i));

    await waitFor(() => {
      expect(screen.getByText(/Location denied/i)).toBeInTheDocument();
    });
  });
});
