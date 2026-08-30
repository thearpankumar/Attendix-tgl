import React, { useState, useEffect } from 'react';
import axios from 'axios';
import {
  Box,
  Card,
  CardContent,
  Typography,
  Chip,
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  IconButton,
  Collapse,
  Paper,
  TextField,
} from '@mui/material';
import {
  ExpandMore as ExpandMoreIcon,
  ExpandLess as ExpandLessIcon,
  Computer as ComputerIcon,
  GpsFixed as GpsIcon,
  Security as SecurityIcon,
  CheckCircle as CheckCircleIcon,
  Cancel as CancelIcon,
  Visibility as VisibilityIcon,
} from '@mui/icons-material';

interface SecurityAnomaly {
  type: string;
  severity: 'low' | 'medium' | 'high';
  details: string;
  detectedAt?: string;
}

interface FlaggedSubmission {
  _id: string;
  rollNumber: string;
  studentName: string;
  capturedAt: string;
  flagged: boolean;
  flagReason: string | null;
  gpsConfidence?: 'high' | 'medium' | 'low' | 'suspicious';
  gpsAnomalies?: SecurityAnomaly[];
  emulatorDetected?: boolean;
  emulatorFlags?: SecurityAnomaly[];
  integrityChecks?: { type: string; details: string }[];
  flagReviewed?: boolean;
  flagReviewedBy?: { username: string } | null;
  flagReviewedAt?: string | null;
}

interface SecuritySummary {
  totalSubmissions: number;
  flaggedSubmissions: number;
  unreviewedFlags: {
    gpsAnomalies: number;
    emulatorDetected: number;
    integrityIssues: number;
  };
  flagPercentage: string;
}

interface AdminSecurityReviewProps {
  sessionId: string;
  apiBaseUrl: string;
}

export default function AdminSecurityReview({ sessionId, apiBaseUrl }: AdminSecurityReviewProps) {
  const [summary, setSummary] = useState<SecuritySummary | null>(null);
  const [flaggedSubmissions, setFlaggedSubmissions] = useState<FlaggedSubmission[]>([]);
  const [loading, setLoading] = useState(true);
  const [expanded, setExpanded] = useState(true);
  const [selectedSubmission, setSelectedSubmission] = useState<FlaggedSubmission | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [reviewDialogOpen, setReviewDialogOpen] = useState(false);
  const [reviewAction, setReviewAction] = useState<'approve' | 'reject' | null>(null);
  const [reviewing, setReviewing] = useState(false);
  const [reviewNotes, setReviewNotes] = useState('');

  useEffect(() => {
    if (!sessionId) return;
    loadSecurityData();
  }, [sessionId]);

  const loadSecurityData = async () => {
    setLoading(true);
    try {
      const [summaryRes, flaggedRes] = await Promise.all([
        axios.get(`${apiBaseUrl}/admin/security/sessions/${sessionId}/security-summary`),
        axios.get(`${apiBaseUrl}/admin/security/sessions/${sessionId}/flagged`),
      ]);

      setSummary(summaryRes.data);
      setFlaggedSubmissions(flaggedRes.data.submissions || []);
    } catch {
      // Failed to load security data
    } finally {
      setLoading(false);
    }
  };

  const handleViewDetails = async (submission: FlaggedSubmission) => {
    try {
      const res = await axios.get(`${apiBaseUrl}/admin/security/attendance/${submission._id}/details`);
      setSelectedSubmission({ ...submission, ...res.data });
      setDetailsOpen(true);
    } catch {
      // Failed to load details
    }
  };

  const handleReview = async () => {
    if (!selectedSubmission || !reviewAction || !reviewNotes.trim()) return;

    setReviewing(true);
    try {
      await axios.post(`${apiBaseUrl}/admin/security/attendance/${selectedSubmission._id}/review`, {
        action: reviewAction,
        notes: reviewNotes.trim(),
      });

      setReviewDialogOpen(false);
      setDetailsOpen(false);
      setReviewNotes('');
      loadSecurityData();
    } catch {
      // Failed to review
    } finally {
      setReviewing(false);
    }
  };

  const getSeverityColor = (severity: string): 'error' | 'warning' | 'default' => {
    switch (severity) {
      case 'high': return 'error';
      case 'medium': return 'warning';
      default: return 'default';
    }
  };

  const getConfidenceColor = (confidence?: string): 'error' | 'warning' | 'success' | 'info' => {
    switch (confidence) {
      case 'suspicious': return 'error';
      case 'low': return 'warning';
      case 'high': return 'success';
      default: return 'info';
    }
  };

  if (loading) {
    return <Card><CardContent><Typography>Loading security data...</Typography></CardContent></Card>;
  }

  if (!summary || summary.flaggedSubmissions === 0) {
    return (
      <Card sx={{ bgcolor: 'var(--color-surface)', color: 'var(--color-text)', border: '1px solid var(--color-border)', mb: 3, boxShadow: 'var(--shadow-card)', borderRadius: 'var(--radius-lg)' }}>
        <Box sx={{ px: 3, py: 1.5, display: "flex", alignItems: "center", gap: 1.5 }}>
          <CheckCircleIcon color="success" fontSize="small" />
          <Typography variant="body1" sx={{ fontWeight: 500, color: 'var(--color-text)' }}>Security Review: No flagged submissions for this session.</Typography>
        </Box>
      </Card>
    );
  }

  return (
    <>
      <Card sx={{ bgcolor: 'var(--color-surface)', color: 'var(--color-text)', border: '1px solid var(--color-border)', mb: 3, boxShadow: 'var(--shadow-card)', borderRadius: 'var(--radius-lg)' }}>
        <Box 
          sx={{ 
            px: 3, 
            py: 1.5, 
            display: 'flex', 
            alignItems: 'center', 
            justifyContent: 'space-between',
            cursor: 'pointer', 
            '&:hover': { bgcolor: 'var(--color-bg-subtle)' } 
          }}
          onClick={() => setExpanded(!expanded)}
        >
          <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
            <SecurityIcon color="warning" fontSize="small" />
            <Typography variant="body1" sx={{ fontWeight: 600, color: 'var(--color-text)' }}>Security Review</Typography>
            <Chip 
              label={`${summary.flaggedSubmissions} flagged`} 
              color="warning" 
              size="small" 
              sx={{ height: 24, fontSize: '0.75rem', fontWeight: 600 }}
            />
          </Box>
          <IconButton size="small" onClick={(e) => { e.stopPropagation(); setExpanded(!expanded); }} sx={{ color: 'var(--color-muted)' }}>
            {expanded ? <ExpandLessIcon /> : <ExpandMoreIcon />}
          </IconButton>
        </Box>

        <Collapse in={expanded}>
          <Box sx={{ px: 3, pb: 3, pt: 1, borderTop: "1px solid var(--color-border)" }}>
            <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 2, mt: 0 }}>
              <Box>
                <Paper sx={{ p: 2, textAlign: 'center', bgcolor: 'var(--color-warning-bg)', border: '1px solid var(--color-warning)', boxShadow: 'none' }}>
                  <Typography variant="h4" sx={{ color: 'var(--color-warning)' }}>
                    {summary.unreviewedFlags.gpsAnomalies}
                  </Typography>
                  <Typography variant="body2" sx={{ color: 'var(--color-warning-txt)' }}>GPS Anomalies</Typography>
                </Paper>
              </Box>
              <Box>
                <Paper sx={{ p: 2, textAlign: 'center', bgcolor: 'var(--color-danger-bg)', border: '1px solid var(--color-danger)', boxShadow: 'none' }}>
                  <Typography variant="h4" sx={{ color: 'var(--color-danger)' }}>
                    {summary.unreviewedFlags.emulatorDetected}
                  </Typography>
                  <Typography variant="body2" sx={{ color: 'var(--color-danger-txt)' }}>Emulator Detected</Typography>
                </Paper>
              </Box>
              <Box>
                <Paper sx={{ p: 2, textAlign: 'center', bgcolor: 'var(--color-primary-light)', border: '1px solid var(--color-primary)', boxShadow: 'none' }}>
                  <Typography variant="h4" sx={{ color: 'var(--color-primary)' }}>
                    {summary.unreviewedFlags.integrityIssues}
                  </Typography>
                  <Typography variant="body2" sx={{ color: 'var(--color-primary)' }}>Integrity Issues</Typography>
                </Paper>
              </Box>
            </Box>

            {summary.flagPercentage !== '0.0' && (
              <Typography variant="body2" sx={{ mt: 2, color: 'var(--color-muted)' }}>
                {summary.flagPercentage}% of submissions flagged for review
              </Typography>
            )}

            <Typography variant="h6" sx={{ mt: 3, mb: 2, color: 'var(--color-text)', fontSize: '15px', fontWeight: 600 }}>Flagged Submissions</Typography>

            <Box sx={{ maxHeight: 400, overflow: 'auto', '&::-webkit-scrollbar': { width: '6px' }, '&::-webkit-scrollbar-thumb': { background: 'var(--color-border)', borderRadius: '999px' } }}>
              {flaggedSubmissions.map((submission) => (
                <Card key={submission._id} sx={{ mb: 1, bgcolor: 'var(--color-bg)', border: '1px solid var(--color-border)', boxShadow: 'none', borderRadius: 'var(--radius-md)' }}>
                  <Box sx={{ px: 2, py: 1.5 }}>
                    <Box sx={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                      <Box sx={{ display: "flex", alignItems: "flex-start", gap: 1.5 }}>
                        <Box sx={{ mt: 0.5 }}>
                          {submission.gpsAnomalies && submission.gpsAnomalies.length > 0 ? (
                            <GpsIcon color="warning" fontSize="small" />
                          ) : submission.emulatorDetected ? (
                            <ComputerIcon color="error" fontSize="small" />
                          ) : (
                            <SecurityIcon color="info" fontSize="small" />
                          )}
                        </Box>
                        <Box>
                          <Typography variant="body2" sx={{ fontWeight: 600, color: 'var(--color-text)' }}>
                            {submission.rollNumber} - {submission.studentName}
                          </Typography>
                          <Box sx={{ display: "flex", gap: 0.5, flexWrap: "wrap", mt: 0.5 }}>
                            {submission.gpsAnomalies?.slice(0, 2).map((a, i) => (
                              <Chip 
                                key={i} 
                                label={a.type} 
                                size="small" 
                                color={getSeverityColor(a.severity)}
                                sx={{ height: 20, fontSize: '0.7rem' }}
                              />
                            ))}
                            {submission.emulatorFlags?.slice(0, 2).map((f, i) => (
                              <Chip 
                                key={i} 
                                label={f.type}
                                size="small" 
                                color="error"
                                sx={{ height: 20, fontSize: '0.7rem' }}
                              />
                            ))}
                            {submission.gpsConfidence && (
                              <Chip 
                                label={`Confidence: ${submission.gpsConfidence}`}
                                size="small"
                                color={getConfidenceColor(submission.gpsConfidence)}
                                sx={{ height: 20, fontSize: '0.7rem' }}
                              />
                            )}
                          </Box>
                        </Box>
                      </Box>
                      <Box sx={{ display: "flex", gap: 1, alignItems: "center" }}>
                        <Chip 
                          label={submission.flagReviewed ? 'Reviewed' : 'Pending'} 
                          size="small"
                          color={submission.flagReviewed ? 'success' : 'warning'}
                          sx={{ height: 20, fontSize: '0.7rem' }}
                        />
                        <IconButton 
                          size="small" 
                          onClick={(e) => { e.stopPropagation(); handleViewDetails(submission); }}
                          sx={{ color: 'var(--color-muted)', '&:hover': { color: 'var(--color-primary)' } }}
                        >
                          <VisibilityIcon fontSize="small" />
                        </IconButton>
                      </Box>
                    </Box>
                  </Box>
                </Card>
              ))}
            </Box>
          </Box>
        </Collapse>
      </Card>

      {/* Details Dialog */}
      <Dialog 
        open={detailsOpen} 
        onClose={() => setDetailsOpen(false)} 
        maxWidth="md" 
        fullWidth
        sx={{ '& .MuiDialog-paper': {
            bgcolor: 'var(--color-surface)',
            color: 'var(--color-text)',
            border: '1px solid var(--color-border)',
            backgroundImage: 'none',
            borderRadius: 'var(--radius-xl)'
          } }}
      >
        <DialogTitle sx={{ color: 'var(--color-text)', fontWeight: 700 }}>Submission Details</DialogTitle>
        <DialogContent dividers sx={{ borderColor: 'var(--color-border)' }}>
          {selectedSubmission && (
            <Box>
              <Typography variant="h6" sx={{ color: 'var(--color-text)', fontWeight: 600 }}>{selectedSubmission.rollNumber} - {selectedSubmission.studentName}</Typography>
              <Typography variant="body2" sx={{ color: 'var(--color-muted)' }}>
                Captured: {new Date(selectedSubmission.capturedAt).toLocaleString()}
              </Typography>

              {selectedSubmission.gpsAnomalies && selectedSubmission.gpsAnomalies.length > 0 && (
                <Box sx={{ mt: 3 }}>
                  <Typography variant="subtitle2" sx={{ color: 'var(--color-text)', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.05em', mb: 1 }}>GPS Anomalies</Typography>
                  {selectedSubmission.gpsAnomalies.map((a, i) => (
                    <Box key={i} sx={{ mt: 1, p: 1.5, bgcolor: 'var(--color-bg)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-md)' }}>
                      <Box sx={{ display: "flex", alignItems: "center", mb: 0.5 }}>
                        <Chip label={a.severity} size="small" color={getSeverityColor(a.severity)} sx={{ mr: 1, height: 20, fontSize: '0.7rem' }} />
                        <Typography variant="body2" sx={{ color: 'var(--color-text)', fontWeight: 600 }}>{a.type}</Typography>
                      </Box>
                      <Typography variant="body2" sx={{ color: 'var(--color-muted)' }}>{a.details}</Typography>
                    </Box>
                  ))}
                </Box>
              )}

              {selectedSubmission.emulatorFlags && selectedSubmission.emulatorFlags.length > 0 && (
                <Box sx={{ mt: 3 }}>
                  <Typography variant="subtitle2" sx={{ color: 'var(--color-text)', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.05em', mb: 1 }}>Emulator Detection</Typography>
                  {selectedSubmission.emulatorFlags.map((f, i) => (
                    <Box key={i} sx={{ mt: 1, p: 1.5, bgcolor: 'var(--color-bg)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-md)' }}>
                      <Box sx={{ display: "flex", alignItems: "center", mb: 0.5 }}>
                        <Chip label={f.severity} size="small" color={getSeverityColor(f.severity)} sx={{ mr: 1, height: 20, fontSize: '0.7rem' }} />
                        <Typography variant="body2" sx={{ color: 'var(--color-text)', fontWeight: 600 }}>{f.type}</Typography>
                      </Box>
                      <Typography variant="body2" sx={{ color: 'var(--color-muted)' }}>{f.details}</Typography>
                    </Box>
                  ))}
                </Box>
              )}

              {selectedSubmission.flagReviewed && (
                <Box sx={{ mt: 3, p: 2, bgcolor: 'var(--color-success-bg)', border: '1px solid var(--color-success)', borderRadius: 'var(--radius-md)' }}>
                  <Typography variant="body2" sx={{ color: 'var(--color-success-txt)' }}>
                    Reviewed by <strong>{selectedSubmission.flagReviewedBy?.username || 'Admin'}</strong> on{' '}
                    {new Date(selectedSubmission.flagReviewedAt || '').toLocaleString()}
                  </Typography>
                </Box>
              )}
            </Box>
          )}
        </DialogContent>
        <DialogActions sx={{ borderTop: '1px solid var(--color-border)', p: 2 }}>
          <Button onClick={() => setDetailsOpen(false)} sx={{ color: 'var(--color-text)' }}>Close</Button>
          {!selectedSubmission?.flagReviewed && (
            <>
              <Button 
                color="success"
                variant="contained" 
                startIcon={<CheckCircleIcon />}
                onClick={() => { setReviewAction('approve'); setReviewDialogOpen(true); }}
                sx={{ boxShadow: 'none' }}
              >
                Mark Safe
              </Button>
              <Button 
                color="error" 
                variant="contained"
                startIcon={<CancelIcon />}
                onClick={() => { setReviewAction('reject'); setReviewDialogOpen(true); }}
                sx={{ boxShadow: 'none' }}
              >
                Reject
              </Button>
            </>
          )}
        </DialogActions>
      </Dialog>

      {/* Review Confirmation Dialog */}
      <Dialog
        open={reviewDialogOpen}
        onClose={() => { setReviewDialogOpen(false); setReviewNotes(''); }}
        sx={{ '& .MuiDialog-paper': {
            bgcolor: 'var(--color-surface)',
            color: 'var(--color-text)',
            border: '1px solid var(--color-border)',
            backgroundImage: 'none',
            borderRadius: 'var(--radius-xl)'
          } }}
      >
        <DialogTitle sx={{ color: 'var(--color-text)', fontWeight: 700 }}>Confirm {reviewAction === 'approve' ? 'Approval' : 'Rejection'}</DialogTitle>
        <DialogContent sx={{ color: 'var(--color-muted)' }}>
          <Typography sx={{ mb: 2 }}>
            Are you sure you want to {reviewAction} this submission?
            {reviewAction === 'approve' && ' This will increase the device trust score.'}
          </Typography>
          <TextField
            autoFocus
            fullWidth
            required
            multiline
            minRows={2}
            label="Review notes"
            placeholder="Why is this being approved/rejected?"
            value={reviewNotes}
            onChange={(e) => setReviewNotes(e.target.value)}
            error={reviewNotes.trim().length === 0}
            helperText={reviewNotes.trim().length === 0 ? 'Notes are required to submit a review.' : ' '}
          />
        </DialogContent>
        <DialogActions sx={{ p: 2 }}>
          <Button onClick={() => { setReviewDialogOpen(false); setReviewNotes(''); }} disabled={reviewing} sx={{ color: 'var(--color-text)' }}>Cancel</Button>
          <Button
            onClick={handleReview}
            color={reviewAction === 'approve' ? 'success' : 'error'}
            variant="contained"
            disabled={reviewing || !reviewNotes.trim()}
            sx={{ boxShadow: 'none' }}
          >
            {reviewing ? 'Processing...' : 'Confirm'}
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
