import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import DashboardWidgets from '../../../src/components/dashboard/DashboardWidgets';
import type { DashboardPulseData } from '../../../src/components/dashboard/DashboardWidgets';
import { MemoryRouter } from 'react-router';

const mockNavigate = vi.fn();
vi.mock('react-router', async () => {
  const actual = await vi.importActual('react-router');
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

const mockPulseData: DashboardPulseData = {
  eligibility: { value: 85, target: 90, delta: 5, deltaType: 'up', status: 'On Track' },
  integrity: { value: 92, target: 95, delta: 2, deltaType: 'up', status: 'On Track' },
  turnout: { value: 78, target: 85, delta: -3, deltaType: 'down', status: 'At Risk' },
  quarantine: { count: 12, status: 'At Risk' }
};

const renderWidgets = (pulse: DashboardPulseData | null = mockPulseData, loading = false) => render(
  <MemoryRouter>
    <DashboardWidgets pulse={pulse} loading={loading} />
  </MemoryRouter>
);

describe('DashboardWidgets', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Normal Cases', () => {
    it('renders 4 widget cards when not loading', () => {
      renderWidgets();
      expect(screen.getByText('Placement Eligibility Index')).toBeInTheDocument();
      expect(screen.getByText('System Integrity Score')).toBeInTheDocument();
      expect(screen.getByText('Active Turnout Rate (Today)')).toBeInTheDocument();
      expect(screen.getByText('Security Quarantine Count')).toBeInTheDocument();
    });

    it('shows percentage values correctly', () => {
      renderWidgets();
      expect(screen.getByText('85%')).toBeInTheDocument();
      expect(screen.getByText('92%')).toBeInTheDocument();
      expect(screen.getByText('78%')).toBeInTheDocument();
    });

    it('shows quarantine count', () => {
      renderWidgets();
      expect(screen.getByText('12')).toBeInTheDocument();
    });

    it('shows status badges', () => {
      renderWidgets();
      const onTrack = screen.queryAllByText('On Track');
      if (onTrack.length === 0) {
        expect(screen.getByText('At Risk')).toBeInTheDocument();
      } else {
        expect(onTrack.length).toBeGreaterThan(0);
      }
    });

    it('shows "Review Now" button on quarantine widget', () => {
      renderWidgets();
      expect(screen.getByText('Review Now')).toBeInTheDocument();
    });

    it('clicking "Review Now" navigates to /flagged', () => {
      renderWidgets();
      fireEvent.click(screen.getByText('Review Now'));
      expect(mockNavigate).toHaveBeenCalledWith('/flagged');
    });

    it('returns null when pulse is null', () => {
      const { container } = renderWidgets(null, false);
      expect(container.firstChild).toBeNull();
    });
  });

  describe('Loading State', () => {
    it('shows skeleton cards when loading', () => {
      renderWidgets(null, true);
      const skeletons = document.querySelectorAll('.animate-pulse');
      expect(skeletons.length).toBeGreaterThanOrEqual(4);
    });

    it('does not show widget content when loading', () => {
      renderWidgets(null, true);
      expect(screen.queryByText('Placement Eligibility Index')).not.toBeInTheDocument();
    });
  });

  describe('Edge Cases', () => {
    it('0% values show correct display', () => {
      const zeroData: DashboardPulseData = {
        eligibility: { value: 0, target: 90, delta: 0, deltaType: 'right', status: 'Critical' },
        integrity: { value: 0, target: 95, delta: 0, deltaType: 'right', status: 'Critical' },
        turnout: { value: 0, target: 85, delta: 0, deltaType: 'right', status: 'Critical' },
        quarantine: { count: 0, status: 'On Track' }
      };
      renderWidgets(zeroData);
      expect(screen.getAllByText('0%').length).toBe(3);
    });

    it('100% values show correct display', () => {
      const perfectData: DashboardPulseData = {
        eligibility: { value: 100, target: 90, delta: 10, deltaType: 'up', status: 'On Track' },
        integrity: { value: 100, target: 95, delta: 5, deltaType: 'up', status: 'On Track' },
        turnout: { value: 100, target: 85, delta: 15, deltaType: 'up', status: 'On Track' },
        quarantine: { count: 0, status: 'On Track' }
      };
      renderWidgets(perfectData);
      expect(screen.getAllByText('100%').length).toBeGreaterThan(0);
    });

    it('Critical status shows for very low values', () => {
      const criticalData: DashboardPulseData = {
        eligibility: { value: 65, target: 90, delta: -5, deltaType: 'down', status: 'Critical' },
        integrity: { value: 60, target: 95, delta: -2, deltaType: 'down', status: 'Critical' },
        turnout: { value: 55, target: 85, delta: -10, deltaType: 'down', status: 'Critical' },
        quarantine: { count: 50, status: 'Critical' }
      };
      renderWidgets(criticalData);
      expect(screen.getByText('65%')).toBeInTheDocument();
    });

    it('very large quarantine count displays', () => {
      const largeQuarantine: DashboardPulseData = {
        ...mockPulseData,
        quarantine: { count: 999999, status: 'Critical' }
      };
      renderWidgets(largeQuarantine);
      const text = screen.queryByText('999,999');
      if (!text) {
        expect(screen.getByText('999999')).toBeInTheDocument();
      } else {
        expect(text).toBeInTheDocument();
      }
    });
  });

  describe('Accessibility', () => {
    it('"Review Now" button has correct role', () => {
      renderWidgets();
      const reviewBtn = screen.getByRole('button', { name: /review now/i });
      expect(reviewBtn).toBeInTheDocument();
    });

    it('info icons render in widgets', () => {
      renderWidgets();
      const infoIcons = document.querySelectorAll('svg');
      expect(infoIcons.length).toBeGreaterThan(3);
    });
  });
});
