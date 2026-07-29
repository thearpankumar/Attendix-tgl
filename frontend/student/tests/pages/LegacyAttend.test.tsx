import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
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
    <MemoryRouter initialEntries={['/s/legacycode']}>
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

  // Regression test: the direct-upload submit body must use `photoPublicId`
  // (matching SubmitAttendanceRequest on the backend), not `publicId`. See
  // backend-rust/src/controllers/attendance.rs's SubmitAttendanceRequest.
  it('direct-upload mode: submits photoPublicId (not publicId) after dev-bypass capture', async () => {
    let submittedBody: any = null;

    (globalThis.fetch as any) = vi.fn().mockImplementation((url: string, init?: RequestInit) => {
      if (url.includes('/api/storage-info')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ provider: 's3', supportsDirectUpload: true }),
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
              isActive: true,
            },
            devBypassEnabled: true,
          }),
        });
      }
      if (url.includes('/captcha')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ captchaId: 'test-captcha', captchaSvg: '<svg></svg>' }),
        });
      }
      if (url.includes('/upload-url')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            uploadUrl: 'https://mock-s3/upload',
            method: 'PUT',
            headers: {},
            publicId: 'mock-public-id-123',
          }),
        });
      }
      if (url.startsWith('data:image')) {
        return Promise.resolve({ blob: () => Promise.resolve(new Blob(['mock'])) });
      }
      if (url === 'https://mock-s3/upload') {
        return Promise.resolve({ ok: true });
      }
      if (url.includes('/submit')) {
        submittedBody = JSON.parse(init!.body as string);
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({
            attendance: { distanceFromLocation: 5, verified: true },
          }),
        });
      }
      return Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText(/Confirm details/i)).toBeInTheDocument());

    // Dev-bypass camera capture (avoids needing a real getUserMedia stream)
    fireEvent.click(screen.getByText(/Use Mock Photo \(DEV\)/i));
    await waitFor(() => expect(screen.getByText(/Retake photo/i)).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Enter your full name/i), { target: { value: 'Alice' } });
    fireEvent.change(screen.getByPlaceholderText(/21CS042/i), { target: { value: 'CS101' } });
    fireEvent.change(screen.getByPlaceholderText(/Enter the code shown/i), { target: { value: 'abcd' } });

    fireEvent.click(screen.getByText(/Submit Attendance/i));

    await waitFor(() => expect(submittedBody).not.toBeNull());
    expect(submittedBody.directUpload).toBe(true);
    expect(submittedBody.photoPublicId).toBe('mock-public-id-123');
    expect(submittedBody.publicId).toBeUndefined();
  });
});
