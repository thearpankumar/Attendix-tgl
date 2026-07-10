import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import App from '../src/App';
import * as useIsMobileModule from '../src/hooks/useIsMobile';

vi.mock('../src/hooks/useIsMobile', () => ({
  useIsMobile: vi.fn(),
}));

// Mock the components so we don't need to render the whole app
vi.mock('../src/components/MobileDeviceRequired', () => ({
  default: () => <div data-testid="mobile-required">Mobile Access Required</div>,
}));

vi.mock('../src/pages/StudentScan', () => ({
  default: () => <div data-testid="student-scan">Student Scan</div>,
}));

vi.mock('../src/pages/LegacyAttend', () => ({
  default: () => <div data-testid="legacy-attend">Legacy Attend</div>,
}));

describe('App', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders MobileDeviceRequired when not on mobile device and bypass is false', () => {
    vi.mocked(useIsMobileModule.useIsMobile).mockReturnValue(false);
    vi.stubEnv('VITE_BYPASS_MOBILE_CHECK', 'false');

    render(<App />);
    
    expect(screen.getByTestId('mobile-required')).toBeInTheDocument();
    expect(screen.queryByTestId('student-scan')).not.toBeInTheDocument();
  });

  it('renders application when on mobile device', () => {
    vi.mocked(useIsMobileModule.useIsMobile).mockReturnValue(true);
    vi.stubEnv('VITE_BYPASS_MOBILE_CHECK', 'false');

    render(<App />);
    
    // It should render the router, and since route is / it will probably render "Not found" 
    // or one of the mocked pages if the route matches. 
    // But MobileDeviceRequired should NOT be there.
    expect(screen.queryByTestId('mobile-required')).not.toBeInTheDocument();
  });

  it('bypasses mobile check and renders application when VITE_BYPASS_MOBILE_CHECK is true', () => {
    vi.mocked(useIsMobileModule.useIsMobile).mockReturnValue(false);
    vi.stubEnv('VITE_BYPASS_MOBILE_CHECK', 'true');

    render(<App />);
    
    expect(screen.queryByTestId('mobile-required')).not.toBeInTheDocument();
  });
});
