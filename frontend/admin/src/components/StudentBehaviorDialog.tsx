import { Dialog, DialogTitle, DialogContent, DialogActions, Button } from '@mui/material';
import { useStudentBehavior } from '../hooks/useStudentBehavior';
import StudentBehaviorPanel from './StudentBehaviorPanel';

/// Per-row "quick view" entry point (Part F of the session-monitoring plan)
/// — a one-shot fetch, no live socket, matching SecurityReview.tsx's
/// existing "click a row, fetch on demand, show in a Dialog" pattern.
export default function StudentBehaviorDialog({
  open,
  onClose,
  sessionId,
  rollNumber,
  studentName,
}: {
  open: boolean;
  onClose: () => void;
  sessionId: string;
  rollNumber: string | null;
  studentName?: string;
}) {
  const { data, loading } = useStudentBehavior(sessionId, open ? rollNumber : null, false);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="sm"
      fullWidth
      sx={{
        '& .MuiDialog-paper': {
          bgcolor: 'var(--color-surface)',
          color: 'var(--color-text)',
          border: '1px solid var(--color-border)',
          backgroundImage: 'none',
          borderRadius: 'var(--radius-xl)',
        },
      }}
    >
      <DialogTitle sx={{ color: 'var(--color-text)', fontWeight: 700 }}>
        Behavior — {studentName || rollNumber}
      </DialogTitle>
      <DialogContent dividers sx={{ borderColor: 'var(--color-border)' }}>
        <StudentBehaviorPanel data={data} loading={loading} />
      </DialogContent>
      <DialogActions sx={{ borderTop: '1px solid var(--color-border)', p: 2 }}>
        <Button onClick={onClose} sx={{ color: 'var(--color-text)' }}>Close</Button>
      </DialogActions>
    </Dialog>
  );
}
