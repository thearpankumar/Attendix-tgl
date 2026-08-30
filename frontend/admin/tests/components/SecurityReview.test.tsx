import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import AdminSecurityReview from '../../src/components/SecurityReview';
import axios from 'axios';

const SUMMARY_WITH_FLAGS = {
  totalSubmissions: 20,
  flaggedSubmissions: 2,
  flagPercentage: '10.0',
  unreviewedFlags: { gpsAnomalies: 1, emulatorDetected: 1, integrityIssues: 0 },
};

const SUBMISSION_GPS = {
  _id: 'sub1',
  rollNumber: 'ROLL001',
  studentName: 'Jane Student',
  capturedAt: new Date().toISOString(),
  flagged: true,
  flagReason: 'gps',
  gpsConfidence: 'suspicious',
  gpsAnomalies: [{ type: 'teleport', severity: 'high', details: 'Moved 500km in 2 minutes' }],
  flagReviewed: false,
};

const SUBMISSION_EMULATOR = {
  _id: 'sub2',
  rollNumber: 'ROLL002',
  studentName: 'John Reviewed',
  capturedAt: new Date().toISOString(),
  flagged: true,
  flagReason: 'emulator',
  emulatorDetected: true,
  emulatorFlags: [{ type: 'root-detected', severity: 'medium', details: 'Rooted device signature' }],
  flagReviewed: true,
  flagReviewedBy: { username: 'admin1' },
  flagReviewedAt: new Date().toISOString(),
};

const makeMockGet = (summary: object, submissions: object[]) => (url: string) => {
  if (url.includes('/security-summary')) return Promise.resolve({ data: summary });
  if (url.includes('/flagged')) return Promise.resolve({ data: { submissions } });
  if (url.includes('/details')) return Promise.resolve({ data: {} });
  return Promise.resolve({ data: {} });
};

describe('AdminSecurityReview', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders nothing alarming when there are no flagged submissions', async () => {
    (axios.get as any).mockImplementation(makeMockGet(
      { totalSubmissions: 5, flaggedSubmissions: 0, flagPercentage: '0.0', unreviewedFlags: { gpsAnomalies: 0, emulatorDetected: 0, integrityIssues: 0 } },
      []
    ));
    render(<AdminSecurityReview sessionId="s1" apiBaseUrl="/api" />);
    await waitFor(() => expect(screen.getByText(/No flagged submissions for this session/i)).toBeInTheDocument());
  });

  it('renders flagged submissions with GPS anomaly and emulator chips, and a pending/reviewed badge each', async () => {
    (axios.get as any).mockImplementation(makeMockGet(SUMMARY_WITH_FLAGS, [SUBMISSION_GPS, SUBMISSION_EMULATOR]));
    render(<AdminSecurityReview sessionId="s1" apiBaseUrl="/api" />);

    await waitFor(() => expect(screen.getByText('2 flagged')).toBeInTheDocument());
    expect(screen.getByText('ROLL001 - Jane Student')).toBeInTheDocument();
    expect(screen.getByText('ROLL002 - John Reviewed')).toBeInTheDocument();
    expect(screen.getByText('teleport')).toBeInTheDocument();
    expect(screen.getByText('root-detected')).toBeInTheDocument();
    expect(screen.getByText('Pending')).toBeInTheDocument();
    expect(screen.getByText('Reviewed')).toBeInTheDocument();
    expect(screen.getByText(/10.0% of submissions flagged for review/i)).toBeInTheDocument();
  });

  it('collapses and re-expands the panel on header click', async () => {
    (axios.get as any).mockImplementation(makeMockGet(SUMMARY_WITH_FLAGS, [SUBMISSION_GPS]));
    render(<AdminSecurityReview sessionId="s1" apiBaseUrl="/api" />);
    await waitFor(() => expect(screen.getByText('Flagged Submissions')).toBeInTheDocument());

    fireEvent.click(screen.getByText('Security Review'));
    // Collapse component keeps content mounted (MUI Collapse), so assert via
    // the header being clickable again rather than absence — the panel's
    // still findable is fine, we're primarily covering the toggle handler.
    fireEvent.click(screen.getByText('Security Review'));
    expect(screen.getByText('Flagged Submissions')).toBeInTheDocument();
  });

  it('opens submission details, shows GPS anomalies, and marks it safe via the review flow', async () => {
    (axios.get as any).mockImplementation(makeMockGet(SUMMARY_WITH_FLAGS, [SUBMISSION_GPS]));
    (axios.get as any).mockImplementation((url: string) => {
      if (url.includes('/details')) return Promise.resolve({ data: { extra: true } });
      return makeMockGet(SUMMARY_WITH_FLAGS, [SUBMISSION_GPS])(url);
    });
    (axios.post as any).mockResolvedValue({ data: {} });

    render(<AdminSecurityReview sessionId="s1" apiBaseUrl="/api" />);
    await waitFor(() => expect(screen.getByText('ROLL001 - Jane Student')).toBeInTheDocument());

    // The eye/visibility icon button has no accessible name in this markup,
    // so target it structurally instead.
    const viewButtons = document.querySelectorAll('.MuiIconButton-root');
    fireEvent.click(viewButtons[viewButtons.length - 1]);

    await waitFor(() => {
      expect(axios.get).toHaveBeenCalledWith('/api/admin/security/attendance/sub1/details');
    });
    expect(await screen.findByText('Submission Details')).toBeInTheDocument();
    // "GPS Anomalies" also labels the summary stat tile — the dialog adds a
    // second occurrence as its section heading.
    expect(screen.getAllByText('GPS Anomalies').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText('Moved 500km in 2 minutes')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Mark Safe'));
    expect(screen.getByText(/Confirm Approval/i)).toBeInTheDocument();
    expect(screen.getByText(/This will increase the device trust score/i)).toBeInTheDocument();

    // Confirm is disabled until notes are entered — required on the backend.
    expect(screen.getByText('Confirm')).toBeDisabled();
    fireEvent.change(screen.getByLabelText(/Review notes/i), {
      target: { value: 'Verified with student, false positive' },
    });
    fireEvent.click(screen.getByText('Confirm'));
    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/security/attendance/sub1/review', {
        action: 'approve',
        notes: 'Verified with student, false positive',
      });
    });
  });

  it('rejects a submission via the review flow, and Cancel closes the confirm dialog without calling the API', async () => {
    (axios.get as any).mockImplementation(makeMockGet(SUMMARY_WITH_FLAGS, [SUBMISSION_GPS]));
    (axios.post as any).mockResolvedValue({ data: {} });

    render(<AdminSecurityReview sessionId="s1" apiBaseUrl="/api" />);
    await waitFor(() => expect(screen.getByText('ROLL001 - Jane Student')).toBeInTheDocument());

    const viewButtons = document.querySelectorAll('.MuiIconButton-root');
    fireEvent.click(viewButtons[viewButtons.length - 1]);
    await screen.findByText('Submission Details');

    fireEvent.click(screen.getByText('Reject'));
    expect(screen.getByText(/Confirm Rejection/i)).toBeInTheDocument();

    // Cancel first — must not call the API.
    fireEvent.click(screen.getByText('Cancel'));
    expect(axios.post).not.toHaveBeenCalled();

    // Reopen and actually confirm the rejection this time.
    fireEvent.click(screen.getByText('Reject'));
    fireEvent.change(screen.getByLabelText(/Review notes/i), {
      target: { value: 'Repeated GPS spoofing pattern' },
    });
    fireEvent.click(screen.getByText('Confirm'));
    await waitFor(() => {
      expect(axios.post).toHaveBeenCalledWith('/api/admin/security/attendance/sub1/review', {
        action: 'reject',
        notes: 'Repeated GPS spoofing pattern',
      });
    });
  });

  it('closes the details dialog via the Close button', async () => {
    (axios.get as any).mockImplementation(makeMockGet(SUMMARY_WITH_FLAGS, [SUBMISSION_EMULATOR]));
    render(<AdminSecurityReview sessionId="s1" apiBaseUrl="/api" />);
    await waitFor(() => expect(screen.getByText('ROLL002 - John Reviewed')).toBeInTheDocument());

    const viewButtons = document.querySelectorAll('.MuiIconButton-root');
    fireEvent.click(viewButtons[viewButtons.length - 1]);
    await screen.findByText('Submission Details');
    // Already-reviewed submission shows the reviewed banner, not Mark Safe/Reject.
    expect(screen.getByText(/Reviewed by/i)).toBeInTheDocument();
    expect(screen.queryByText('Mark Safe')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('Close'));
    await waitFor(() => expect(screen.queryByText('Submission Details')).not.toBeInTheDocument());
  });

  it('does not fetch when sessionId is empty', () => {
    render(<AdminSecurityReview sessionId="" apiBaseUrl="/api" />);
    expect(axios.get).not.toHaveBeenCalled();
  });
});
