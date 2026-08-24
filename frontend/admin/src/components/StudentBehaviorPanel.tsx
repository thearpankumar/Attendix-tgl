import { Box, Typography, Chip, CircularProgress } from '@mui/material';
import type { StudentBehaviorResponse } from '../hooks/useStudentBehavior';

const EVENT_LABELS: Record<string, string> = {
  extension_lifecycle: 'Extension install/uninstall',
  heartbeat: 'Heartbeat',
  idle_state: 'Idle state change',
  tab_visibility: 'Tab visibility change',
  window_focus: 'Window focus change',
  input_activity: 'Input activity',
  tab_url_change: 'Tab / URL change',
  tab_count: 'Tab count change',
  tab_group: 'Tab group change',
  navigation_history: 'Navigation',
  window_count: 'Window count change',
  target_tab_focus: 'Target tab focus change',
  meet_tab_open: 'Meet tab open/closed',
  device_fingerprint: 'Device fingerprint',
  ua_high_entropy: 'Browser/OS details',
  ip_webrtc_check: 'IP / WebRTC check',
  emulator_flag: 'Automation signal',
  multi_monitor: 'Multiple monitors',
};

const eventLabel = (eventType: string) => EVENT_LABELS[eventType] || eventType;

const LOCK_STATUS_TONE: Record<string, 'success' | 'default' | 'warning'> = {
  active: 'success',
  released: 'default',
  superseded: 'warning',
};

/// Shared by the per-row quick-view dialog and the top-panel deep-dive
/// dashboard — device-lock status, derived summary stats, and the raw event
/// timeline, all driven by one `useStudentBehavior` fetch.
export default function StudentBehaviorPanel({
  data,
  loading,
}: {
  data: StudentBehaviorResponse | null;
  loading: boolean;
}) {
  if (loading && !data) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', p: 4 }}>
        <CircularProgress size={28} />
      </Box>
    );
  }

  if (!data) {
    return (
      <Typography variant="body2" sx={{ color: 'var(--color-muted)', p: 2 }}>
        No behavior data available.
      </Typography>
    );
  }

  const { summary, deviceLocks, events } = data;

  return (
    <Box>
      <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', mb: 3 }}>
        <Chip label={`${summary.eventCount} events`} size="small" />
        <Chip label={`${Math.round(summary.totalIdleSeconds / 60)} min idle`} size="small" color={summary.totalIdleSeconds > 300 ? 'warning' : 'default'} />
        <Chip label={`${summary.focusTransitionCount} focus changes`} size="small" />
        <Chip label={`${summary.tabSwitchCount} tab switches`} size="small" />
        <Chip
          label={summary.meetTabEverOpen ? 'Meet tab opened' : 'Meet tab never opened'}
          size="small"
          color={summary.meetTabEverOpen ? 'success' : 'error'}
        />
      </Box>

      <Typography variant="subtitle2" sx={{ color: 'var(--color-text)', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.05em', mb: 1 }}>
        Device Locks
      </Typography>
      {deviceLocks.length === 0 ? (
        <Typography variant="body2" sx={{ color: 'var(--color-muted)', mb: 3 }}>
          No device has paired for this session yet.
        </Typography>
      ) : (
        <Box sx={{ mb: 3 }}>
          {deviceLocks.map((lock) => (
            <Box
              key={lock.id}
              sx={{ display: 'flex', alignItems: 'center', gap: 1, mt: 1, p: 1.25, bgcolor: 'var(--color-bg)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-md)' }}
            >
              <Chip label={lock.status} size="small" color={LOCK_STATUS_TONE[lock.status] || 'default'} />
              <Typography variant="body2" sx={{ color: 'var(--color-muted)', fontFamily: 'monospace', fontSize: '0.75rem' }}>
                {lock.extensionInstanceId.slice(0, 8)}…
              </Typography>
              <Typography variant="body2" sx={{ color: 'var(--color-muted)', ml: 'auto' }}>
                {new Date(lock.lockedAt).toLocaleString()}
              </Typography>
            </Box>
          ))}
        </Box>
      )}

      <Typography variant="subtitle2" sx={{ color: 'var(--color-text)', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.05em', mb: 1 }}>
        Timeline
      </Typography>
      {events.length === 0 ? (
        <Typography variant="body2" sx={{ color: 'var(--color-muted)' }}>
          No telemetry recorded yet.
        </Typography>
      ) : (
        <Box sx={{ maxHeight: 320, overflowY: 'auto' }}>
          {[...events].reverse().map((event, i) => (
            <Box
              key={i}
              sx={{ display: 'flex', justifyContent: 'space-between', gap: 2, py: 0.75, borderBottom: '1px solid var(--color-border)' }}
            >
              <Typography variant="body2" sx={{ color: 'var(--color-text)' }}>{eventLabel(event.eventType)}</Typography>
              <Typography variant="body2" sx={{ color: 'var(--color-muted)', whiteSpace: 'nowrap' }}>
                {new Date(event.recordedAt).toLocaleTimeString()}
              </Typography>
            </Box>
          ))}
        </Box>
      )}
    </Box>
  );
}
