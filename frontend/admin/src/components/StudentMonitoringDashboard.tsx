import { useMemo, useState } from 'react';
import { Dialog, DialogTitle, DialogContent, DialogActions, Button } from '@mui/material';
import {
  ScatterChart, Scatter, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer,
} from 'recharts';
import { useStudentBehavior } from '../hooks/useStudentBehavior';
import StudentBehaviorPanel from './StudentBehaviorPanel';

export interface MonitoredStudent {
  rollNumber: string;
  studentName: string;
}

const CHART_CATEGORIES = ['idle_state', 'window_focus', 'tab_url_change', 'meet_tab_open'];
const CHART_CATEGORY_LABELS: Record<string, string> = {
  idle_state: 'Idle',
  window_focus: 'Focus',
  tab_url_change: 'Tab/URL',
  meet_tab_open: 'Meet tab',
};

/// Top-panel "Monitoring" deep-dive (Part F of the session-monitoring plan)
/// — a student picker + a live recharts timeline, connected to the WebSocket
/// from useStudentBehavior only while this dashboard is open (see that
/// hook's `live` param).
export default function StudentMonitoringDashboard({
  open,
  onClose,
  sessionId,
  students,
  initialRollNumber,
}: {
  open: boolean;
  onClose: () => void;
  sessionId: string;
  students: MonitoredStudent[];
  initialRollNumber?: string | null;
}) {
  const [rollNumber, setRollNumber] = useState<string | null>(initialRollNumber ?? students[0]?.rollNumber ?? null);
  const { data, loading } = useStudentBehavior(sessionId, open ? rollNumber : null, open);

  const chartData = useMemo(() => {
    if (!data) return [];
    return data.events
      .filter((e) => CHART_CATEGORIES.includes(e.eventType))
      .map((e) => ({
        x: new Date(e.recordedAt).getTime(),
        y: CHART_CATEGORIES.indexOf(e.eventType),
        eventType: e.eventType,
        time: e.recordedAt,
      }));
  }, [data]);

  const studentName = students.find((s) => s.rollNumber === rollNumber)?.studentName;

  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="lg"
      fullWidth
      sx={{
        '& .MuiDialog-paper': {
          bgcolor: 'var(--color-surface)',
          color: 'var(--color-text)',
          border: '1px solid var(--color-border)',
          backgroundImage: 'none',
          borderRadius: 'var(--radius-xl)',
          minHeight: '60vh',
        },
      }}
    >
      <DialogTitle sx={{ color: 'var(--color-text)', fontWeight: 700, display: 'flex', alignItems: 'center', gap: 2 }}>
        <span>Monitoring</span>
        <select
          value={rollNumber || ''}
          onChange={(e) => setRollNumber(e.target.value || null)}
          style={{ fontSize: '0.85rem', padding: '0.3rem 0.5rem' }}
        >
          <option value="" disabled>Select a student…</option>
          {students.map((s) => (
            <option key={s.rollNumber} value={s.rollNumber}>{s.studentName} ({s.rollNumber})</option>
          ))}
        </select>
      </DialogTitle>
      <DialogContent dividers sx={{ borderColor: 'var(--color-border)' }}>
        {!rollNumber ? (
          <p style={{ color: 'var(--color-muted)' }}>Select a student to view their session behavior.</p>
        ) : (
          <>
            <div style={{ width: '100%', height: 180, marginBottom: '1.5rem' }}>
              <ResponsiveContainer>
                <ScatterChart margin={{ top: 10, right: 20, bottom: 10, left: 10 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
                  <XAxis
                    dataKey="x"
                    type="number"
                    domain={['dataMin', 'dataMax']}
                    tickFormatter={(t) => new Date(t).toLocaleTimeString()}
                    tick={{ fill: 'var(--color-muted)', fontSize: 11 }}
                  />
                  <YAxis
                    dataKey="y"
                    type="number"
                    domain={[-0.5, CHART_CATEGORIES.length - 0.5]}
                    ticks={CHART_CATEGORIES.map((_, i) => i)}
                    tickFormatter={(i) => CHART_CATEGORY_LABELS[CHART_CATEGORIES[i]] || ''}
                    tick={{ fill: 'var(--color-muted)', fontSize: 11 }}
                    width={70}
                  />
                  <Tooltip
                    formatter={(_value, _name, props) => [CHART_CATEGORY_LABELS[props.payload.eventType], 'Event']}
                    labelFormatter={(t) => new Date(t as number).toLocaleTimeString()}
                  />
                  <Scatter data={chartData} fill="var(--color-primary, #4f46e5)" />
                </ScatterChart>
              </ResponsiveContainer>
            </div>
            <StudentBehaviorPanel data={data} loading={loading} />
          </>
        )}
      </DialogContent>
      <DialogActions sx={{ borderTop: '1px solid var(--color-border)', p: 2 }}>
        {studentName && (
          <span style={{ marginRight: 'auto', color: 'var(--color-muted)', fontSize: '0.8rem' }}>
            Live while this panel is open
          </span>
        )}
        <Button onClick={onClose} sx={{ color: 'var(--color-text)' }}>Close</Button>
      </DialogActions>
    </Dialog>
  );
}
