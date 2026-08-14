import { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import Modal from '../components/ui/Modal';
import Button from '../components/ui/Button';
import {
  ShieldAlert, Satellite, Cpu, Star, Zap, KeyRound, Camera,
  Info, RotateCcw, Save, CheckCircle2, Settings as SettingsIcon, Lock
} from 'lucide-react';

// ─── Types ───────────────────────────────────────────────────────────────────

interface GpsValidationConfig {
  enabled: boolean;
  accuracyVerySuspicious: number;
  accuracySuspicious: number;
  speedThreshold: number;
  timestampDriftMax: number;
  positionJumpThreshold: number;
  geofenceMaxDistanceM: number;
  altitudeZeroPenalty: boolean;
}

interface EmulatorDetectionConfig {
  enabled: boolean;
  blockOnHighSeverity: boolean;
}

interface TrustScoreConfig {
  anomalyPenalty: number;
  safeReviewBonus: number;
}

interface RateLimitsConfig {
  adminWindowSecs: number;
  adminMaxRequests: number;
  studentWindowSecs: number;
  studentMaxRequests: number;
  loginWindowSecs: number;
  loginMaxRequests: number;
  clientLogWindowSecs: number;
  clientLogMaxRequests: number;
}

interface WebAuthnSystemConfig {
  gracePeriodMinutes: number;
}

interface PhotoVerificationConfig {
  similarityThreshold: number;
  highSimilarityThreshold: number;
}

interface SessionConfig {
  expireMinutes: number;
}

interface LockoutConfig {
  maxLoginAttempts: number;
  lockoutDurationMinutes: number;
}

interface AttendanceConfig {
  maxAttendanceAttempts: number;
}

interface SystemConfig {
  devBypassEnabled: boolean;
  gpsValidation: GpsValidationConfig;
  emulatorDetection: EmulatorDetectionConfig;
  trustScore: TrustScoreConfig;
  rateLimits: RateLimitsConfig;
  webauthnConfig: WebAuthnSystemConfig;
  photoVerification: PhotoVerificationConfig;
  sessionConfig: SessionConfig;
  lockoutConfig: LockoutConfig;
  attendanceConfig: AttendanceConfig;
}

// ─── Defaults (mirrors Rust defaults) ────────────────────────────────────────

const DEFAULT_CONFIG: SystemConfig = {
  devBypassEnabled: false,
  gpsValidation: {
    enabled: true,
    accuracyVerySuspicious: 3,
    accuracySuspicious: 10,
    speedThreshold: 50,
    timestampDriftMax: 60000,
    positionJumpThreshold: 500,
    geofenceMaxDistanceM: 100,
    altitudeZeroPenalty: true,
  },
  emulatorDetection: { enabled: true, blockOnHighSeverity: false },
  trustScore: { anomalyPenalty: 15, safeReviewBonus: 10 },
  rateLimits: {
    adminWindowSecs: 60, adminMaxRequests: 1000,
    studentWindowSecs: 60, studentMaxRequests: 100,
    loginWindowSecs: 60, loginMaxRequests: 20,
    clientLogWindowSecs: 60, clientLogMaxRequests: 100,
  },
  webauthnConfig: { gracePeriodMinutes: 15 },
  photoVerification: { similarityThreshold: 0.15, highSimilarityThreshold: 0.85 },
  sessionConfig: { expireMinutes: 60 },
  lockoutConfig: { maxLoginAttempts: 5, lockoutDurationMinutes: 15 },
  attendanceConfig: { maxAttendanceAttempts: 3 },
};

// ─── Tab definitions ──────────────────────────────────────────────────────────

type TabId = 'security' | 'gps' | 'emulator' | 'trust' | 'ratelimit' | 'webauthn' | 'photo';

const TABS: { id: TabId; label: string; icon: React.ReactNode; color: string; count: number }[] = [
  { id: 'security',  label: 'Security',           icon: <ShieldAlert size={16} />, color: '#ef4444', count: 1 },
  { id: 'gps',       label: 'GPS Validation',      icon: <Satellite size={16} />,  color: '#06b6d4', count: 7 },
  { id: 'emulator',  label: 'Emulator Detection',  icon: <Cpu size={16} />,        color: '#8b5cf6', count: 2 },
  { id: 'trust',     label: 'Trust Score',         icon: <Star size={16} />,       color: '#22c55e', count: 2 },
  { id: 'ratelimit', label: 'Rate Limiting',       icon: <Zap size={16} />,        color: '#f97316', count: 8 },
  { id: 'webauthn',  label: 'WebAuthn',            icon: <KeyRound size={16} />,   color: '#eab308', count: 1 },
  { id: 'photo',     label: 'Photo Verification',  icon: <Camera size={16} />,     color: '#ec4899', count: 2 },
];

// ─── Tiny helpers ─────────────────────────────────────────────────────────────

function Toggle({ checked, onChange, color = '#06b6d4' }: { checked: boolean; onChange: (v: boolean) => void; color?: string }) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      style={{
        width: 44, height: 24, borderRadius: 12, border: 'none', cursor: 'pointer', position: 'relative',
        background: checked ? color : 'var(--color-border)',
        transition: 'background 0.2s',
        flexShrink: 0,
        boxShadow: checked ? `0 0 8px ${color}60` : 'none',
      }}
    >
      <span style={{
        position: 'absolute', top: 3, left: checked ? 23 : 3, width: 18, height: 18,
        borderRadius: '50%', background: '#fff',
        transition: 'left 0.2s', boxShadow: '0 1px 3px rgba(0,0,0,0.3)',
      }} />
    </button>
  );
}

function NumInput({ value, onChange, unit, min, max, step = 1 }: {
  value: number; onChange: (v: number) => void; unit?: string; min?: number; max?: number; step?: number
}) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 0 }}>
      <input
        type="number" value={value} min={min} max={max} step={step}
        onChange={e => onChange(Number(e.target.value))}
        style={{
          width: 90, padding: '6px 10px', borderRadius: unit ? '6px 0 0 6px' : 6,
          background: 'var(--color-bg-subtle)', border: '1px solid var(--color-border)',
          color: 'var(--color-text)', fontSize: 14, outline: 'none',
          borderRight: unit ? 'none' : undefined,
        }}
      />
      {unit && (
        <span style={{
          padding: '6px 10px', background: 'var(--color-bg-subtle)',
          border: '1px solid var(--color-border)', borderLeft: 'none',
          borderRadius: '0 6px 6px 0', color: 'var(--color-muted)', fontSize: 13, whiteSpace: 'nowrap',
        }}>{unit}</span>
      )}
    </div>
  );
}

function SettingRow({ label, desc, tooltip, children }: { label: string; desc?: string; tooltip?: string; children: React.ReactNode }) {
  const [tip, setTip] = useState(false);
  return (
    <div className="settings-row">
      <div className="settings-row-info">
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--color-text)' }}>{label}</span>
          {tooltip && (
            <span style={{ position: 'relative', display: 'inline-flex' }}>
              <Info
                size={13} style={{ color: 'var(--color-muted)', cursor: 'pointer' }}
                onMouseEnter={() => setTip(true)} onMouseLeave={() => setTip(false)}
              />
              {tip && (
                <div style={{
                  position: 'absolute', bottom: '120%', left: '50%', transform: 'translateX(-50%)',
                  background: 'var(--color-surface)', border: '1px solid var(--color-border)',
                  borderRadius: 8, padding: '8px 12px', fontSize: 12, color: 'var(--color-text)',
                  zIndex: 100, boxShadow: 'var(--shadow-lg)',
                  maxWidth: 260, whiteSpace: 'pre-wrap',
                }}>{tooltip}</div>
              )}
            </span>
          )}
        </div>
        {desc && <p style={{ margin: '4px 0 0', fontSize: 12, color: 'var(--color-muted)', lineHeight: 1.5 }}>{desc}</p>}
      </div>
      <div className="settings-row-control">{children}</div>
    </div>
  );
}

// ─── Section panels ───────────────────────────────────────────────────────────

function SecuritySection({ config, onChange }: { config: SystemConfig; onChange: (c: SystemConfig) => void }) {
  const [showModal, setShowModal] = useState(false);
  const [password, setPassword] = useState('');
  const [pending, setPending] = useState(false);

  const handleToggle = () => setShowModal(true);

  const confirmToggle = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!password) { toast.error('Password required'); return; }
    setPending(true);
    try {
      const res = await axios.post('/api/config/dev-bypass', { enabled: !config.devBypassEnabled, password });
      onChange({ ...config, devBypassEnabled: res.data.config.devBypassEnabled });
      toast.success(res.data.message);
      setShowModal(false); setPassword('');
    } catch (err: unknown) {
      toast.error(axios.isAxiosError(err) ? (err.response?.data?.message || 'Failed') : 'Failed');
    } finally { setPending(false); }
  };

  const on = config.devBypassEnabled;
  return (
    <>
      <SettingRow
        label="Developer Security Bypass"
        desc="Relaxes all hardware security checks and injects mock buttons into the student app. Bypassed records are permanently flagged."
        tooltip="Enable only for development/testing. Never in production."
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: on ? '#ef4444' : '#22c55e', whiteSpace: 'nowrap' }}>
            {on ? 'ENABLED — SYSTEM VULNERABLE' : 'DISABLED — STRICT'}
          </span>
          <button
            onClick={handleToggle}
            style={{
              padding: '8px 18px', borderRadius: 8, border: 'none', cursor: 'pointer', fontWeight: 600,
              fontSize: 13, background: on ? '#ef4444' : 'rgba(239,68,68,0.15)',
              color: on ? '#fff' : '#ef4444', transition: 'all 0.2s', flexShrink: 0,
            }}
          >{on ? 'Disable' : 'Enable'}</button>
        </div>
      </SettingRow>

      <SettingRow
        label="Admin Session Expiry"
        tooltip="How long an admin can stay logged in without activity before the session expires."
      >
        <NumInput value={config.sessionConfig.expireMinutes} onChange={v => onChange({ ...config, sessionConfig: { ...config.sessionConfig, expireMinutes: v } })} unit="minutes" min={1} step={5} />
      </SettingRow>

      <SettingRow
        label="Admin Lockout Duration"
        tooltip="How long an admin account remains locked after exceeding max login attempts."
      >
        <NumInput value={config.lockoutConfig.lockoutDurationMinutes} onChange={v => onChange({ ...config, lockoutConfig: { ...config.lockoutConfig, lockoutDurationMinutes: v } })} unit="minutes" min={1} step={5} />
      </SettingRow>

      <SettingRow
        label="Max Admin Login Attempts"
        tooltip="Maximum failed login attempts before an admin account is locked."
      >
        <NumInput value={config.lockoutConfig.maxLoginAttempts} onChange={v => onChange({ ...config, lockoutConfig: { ...config.lockoutConfig, maxLoginAttempts: v } })} min={1} />
      </SettingRow>

      <SettingRow
        label="Max Attendance Attempts"
        tooltip="Maximum failed check-in attempts a student can make in one session."
      >
        <NumInput value={config.attendanceConfig.maxAttendanceAttempts} onChange={v => onChange({ ...config, attendanceConfig: { ...config.attendanceConfig, maxAttendanceAttempts: v } })} min={1} />
      </SettingRow>

      <Modal open={showModal} onClose={() => { setShowModal(false); setPassword(''); }} title="Confirm Security Change">
        <form onSubmit={confirmToggle}>
          <p style={{ fontSize: 14, color: 'var(--color-muted)', marginBottom: 16, lineHeight: 1.6 }}>
            You are about to <strong style={{ color: on ? '#22c55e' : '#ef4444' }}>
              {on ? 'DISABLE' : 'ENABLE'}</strong> Developer Security Bypass.
            Enter your administrator password to confirm.
          </p>
          <div className="form-group">
            <label>Administrator Password</label>
            <input type="password" className="form-control" value={password}
              onChange={e => setPassword(e.target.value)} placeholder="Enter password" autoFocus required />
          </div>
          <div style={{ display: 'flex', gap: 12, justifyContent: 'flex-end', marginTop: 20 }}>
            <Button type="button" variant="secondary" onClick={() => { setShowModal(false); setPassword(''); }}>Cancel</Button>
            <Button type="submit" variant={on ? 'primary' : 'danger'} disabled={pending}>
              {pending ? 'Confirming…' : 'Confirm'}
            </Button>
          </div>
        </form>
      </Modal>
    </>
  );
}

function GpsSection({ cfg, onChange }: { cfg: GpsValidationConfig; onChange: (c: GpsValidationConfig) => void }) {
  const s = (key: keyof GpsValidationConfig) => (val: number | boolean) => onChange({ ...cfg, [key]: val });
  return (
    <>
      <SettingRow label="GPS Validation Enabled" desc="Master switch for all GPS-based security checks.">
        <Toggle checked={cfg.enabled} onChange={s('enabled') as (v: boolean) => void} color="#06b6d4" />
      </SettingRow>
      <SettingRow label="Accuracy — Very Suspicious" tooltip="GPS readings more precise than this (smaller) are flagged as highly suspicious (e.g., mocked location).">
        <NumInput value={cfg.accuracyVerySuspicious} onChange={s('accuracyVerySuspicious') as (v: number) => void} unit="meters" min={0.1} step={0.5} />
      </SettingRow>
      <SettingRow label="Accuracy — Suspicious" tooltip="GPS readings between this and 'Very Suspicious' threshold get a medium suspicion flag.">
        <NumInput value={cfg.accuracySuspicious} onChange={s('accuracySuspicious') as (v: number) => void} unit="meters" min={1} step={1} />
      </SettingRow>
      <SettingRow label="Speed Threshold" tooltip="Student movement faster than this between checks triggers a suspicious speed flag.">
        <NumInput value={cfg.speedThreshold} onChange={s('speedThreshold') as (v: number) => void} unit="km/h" min={0} step={5} />
      </SettingRow>
      <SettingRow label="Timestamp Drift Max" tooltip="Maximum allowed difference between GPS timestamp and server time before flagging clock manipulation.">
        <NumInput value={cfg.timestampDriftMax} onChange={s('timestampDriftMax') as (v: number) => void} unit="ms" min={0} step={1000} />
      </SettingRow>
      <SettingRow label="Position Jump Threshold" tooltip="Maximum distance between two consecutive GPS readings before flagging a teleport/jump.">
        <NumInput value={cfg.positionJumpThreshold} onChange={s('positionJumpThreshold') as (v: number) => void} unit="meters" min={0} step={10} />
      </SettingRow>
      <SettingRow label="Geofence Max Distance" tooltip="Maximum allowed distance in meters from the designated location for a valid attendance check-in.">
        <NumInput value={cfg.geofenceMaxDistanceM} onChange={s('geofenceMaxDistanceM') as (v: number) => void} unit="meters" min={1} step={5} />
      </SettingRow>
      <SettingRow label="Altitude Zero Penalty" desc="Flag submissions where altitude is zero or null (common in mock GPS providers).">
        <Toggle checked={cfg.altitudeZeroPenalty} onChange={s('altitudeZeroPenalty') as (v: boolean) => void} color="#06b6d4" />
      </SettingRow>
    </>
  );
}

function EmulatorSection({ cfg, onChange }: { cfg: EmulatorDetectionConfig; onChange: (c: EmulatorDetectionConfig) => void }) {
  const s = (key: keyof EmulatorDetectionConfig) => (val: boolean) => onChange({ ...cfg, [key]: val });
  return (
    <>
      <SettingRow label="Emulator Detection Enabled" desc="Master switch for browser/device emulator detection signals.">
        <Toggle checked={cfg.enabled} onChange={s('enabled')} color="#8b5cf6" />
      </SettingRow>
      <SettingRow label="Block on High Severity" desc="Automatically reject submissions with HIGH severity emulator signals (desktop GPU, WebGL emulator renderer, etc.)." tooltip="Low and medium severity flags are still recorded but don't block submission. High severity indicates strong evidence of emulation.">
        <Toggle checked={cfg.blockOnHighSeverity} onChange={s('blockOnHighSeverity')} color="#8b5cf6" />
      </SettingRow>
    </>
  );
}

function TrustSection({ cfg, onChange }: { cfg: TrustScoreConfig; onChange: (c: TrustScoreConfig) => void }) {
  const s = (key: keyof TrustScoreConfig) => (val: number) => onChange({ ...cfg, [key]: val });
  return (
    <>
      <SettingRow label="Anomaly Penalty" desc="Trust score points deducted per detected anomaly (GPS, emulator, integrity flags)." tooltip="Each flagged attendance submission loses this many trust points. Higher = stricter.">
        <NumInput value={cfg.anomalyPenalty} onChange={s('anomalyPenalty')} unit="pts" min={0} max={100} step={1} />
      </SettingRow>
      <SettingRow label="Safe Review Bonus" desc="Trust points restored when an admin manually approves a flagged submission." tooltip="Rewards legitimate students whose flags were incorrectly triggered.">
        <NumInput value={cfg.safeReviewBonus} onChange={s('safeReviewBonus')} unit="pts" min={0} max={100} step={1} />
      </SettingRow>
    </>
  );
}

function RateLimitSection({ cfg, onChange }: { cfg: RateLimitsConfig; onChange: (c: RateLimitsConfig) => void }) {
  const s = (key: keyof RateLimitsConfig) => (val: number) => onChange({ ...cfg, [key]: val });
  const Row = ({ label, winKey, maxKey, desc }: { label: string; winKey: keyof RateLimitsConfig; maxKey: keyof RateLimitsConfig; desc?: string }) => (
    <SettingRow label={label} desc={desc}>
      <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
        <div style={{ textAlign: 'right' }}>
          <div style={{ fontSize: 11, color: 'var(--color-muted)', marginBottom: 4 }}>Window</div>
          <NumInput value={cfg[winKey] as number} onChange={s(winKey)} unit="sec" min={1} />
        </div>
        <div style={{ textAlign: 'right' }}>
          <div style={{ fontSize: 11, color: 'var(--color-muted)', marginBottom: 4 }}>Max Req</div>
          <NumInput value={cfg[maxKey] as number} onChange={s(maxKey)} min={1} />
        </div>
      </div>
    </SettingRow>
  );
  return (
    <>
      <Row label="Admin API" winKey="adminWindowSecs" maxKey="adminMaxRequests" desc="Rate limit for authenticated admin dashboard endpoints." />
      <Row label="Student API" winKey="studentWindowSecs" maxKey="studentMaxRequests" desc="Rate limit for student attendance submission endpoints." />
      <Row label="Login / Auth" winKey="loginWindowSecs" maxKey="loginMaxRequests" desc="Rate limit for login and registration endpoints." />
      <Row label="Client Logs" winKey="clientLogWindowSecs" maxKey="clientLogMaxRequests" desc="Rate limit for client-side error log reporting." />
    </>
  );
}

function WebAuthnSection({ cfg, onChange }: { cfg: WebAuthnSystemConfig; onChange: (c: WebAuthnSystemConfig) => void }) {
  return (
    <SettingRow label="Grace Period After Enrollment" desc="How long after WebAuthn enrollment before biometric authentication is enforced on attendance." tooltip="During the grace period, students can still submit without biometric verification. This is useful for onboarding.">
      <NumInput value={cfg.gracePeriodMinutes} onChange={v => onChange({ ...cfg, gracePeriodMinutes: v })} unit="min" min={0} max={1440} step={5} />
    </SettingRow>
  );
}

function PhotoSection({ cfg, onChange }: { cfg: PhotoVerificationConfig; onChange: (c: PhotoVerificationConfig) => void }) {
  return (
    <>
      <SettingRow label="Similarity Threshold (Distinct)" desc="If two photos score below this value, they are considered completely different images." tooltip="Scale: 0.0 = completely different, 1.0 = identical. Photos below this score are confidently accepted as unique.">
        <NumInput value={cfg.similarityThreshold} onChange={v => onChange({ ...cfg, similarityThreshold: v })} min={0} max={1} step={0.01} />
      </SettingRow>
      <SettingRow label="High Similarity Threshold (Duplicate)" desc="Photos scoring above this are flagged as reused/duplicate — possible photo sharing between students." tooltip="Scale: 0.0 = completely different, 1.0 = identical. Photos above this score trigger a duplication flag.">
        <NumInput value={cfg.highSimilarityThreshold} onChange={v => onChange({ ...cfg, highSimilarityThreshold: v })} min={0} max={1} step={0.01} />
      </SettingRow>
      <div style={{ marginTop: 16, padding: '12px 16px', borderRadius: 8, background: 'rgba(236,72,153,0.08)', border: '1px solid rgba(236,72,153,0.2)' }}>
        <div style={{ fontSize: 12, color: 'var(--color-muted)', lineHeight: 1.6 }}>
          <strong style={{ color: '#ec4899' }}>Current thresholds:</strong>{' '}
          Score &lt; <strong>{cfg.similarityThreshold}</strong> → Unique &nbsp;·&nbsp;
          <strong>{cfg.similarityThreshold}</strong> – <strong>{cfg.highSimilarityThreshold}</strong> → Review &nbsp;·&nbsp;
          Score &gt; <strong>{cfg.highSimilarityThreshold}</strong> → Duplicate Flag
        </div>
      </div>
    </>
  );
}

// ─── Account (password change) ─────────────────────────────────────────────────

function AccountSection() {
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newPassword.length < 6) { toast.error('New password must be at least 6 characters'); return; }
    if (newPassword !== confirmPassword) { toast.error('New passwords do not match'); return; }
    setSaving(true);
    try {
      await axios.patch('/api/admin/profile/password', { currentPassword, newPassword });
      toast.success('Password updated successfully');
      setCurrentPassword(''); setNewPassword(''); setConfirmPassword('');
    } catch (err: unknown) {
      toast.error(axios.isAxiosError(err) ? (err.response?.data?.message || 'Failed to update password') : 'Failed to update password');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="card" style={{ marginBottom: 24, padding: '1.25rem 1.5rem' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 16 }}>
        <div style={{ width: 36, height: 36, borderRadius: 10, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(79,70,229,0.12)', color: 'var(--color-primary)' }}>
          <Lock size={17} />
        </div>
        <div>
          <div style={{ fontSize: 15, fontWeight: 600, color: 'var(--color-text)' }}>Change Password</div>
          <div style={{ fontSize: 12, color: 'var(--color-muted)' }}>Update your own login password</div>
        </div>
      </div>
      <form onSubmit={handleSubmit} style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: 12, alignItems: 'end' }}>
        <div className="form-group" style={{ margin: 0 }}>
          <label>Current Password</label>
          <input type="password" value={currentPassword} onChange={(e) => setCurrentPassword(e.target.value)} required />
        </div>
        <div className="form-group" style={{ margin: 0 }}>
          <label>New Password</label>
          <input type="password" value={newPassword} onChange={(e) => setNewPassword(e.target.value)} minLength={6} required />
        </div>
        <div className="form-group" style={{ margin: 0 }}>
          <label>Confirm New Password</label>
          <input type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)} minLength={6} required />
        </div>
        <Button type="submit" variant="primary" disabled={saving} style={{ height: 38 }}>{saving ? 'Saving…' : 'Update Password'}</Button>
      </form>
    </div>
  );
}

// ─── Main component ───────────────────────────────────────────────────────────

const Settings = () => {
  const [activeTab, setActiveTab] = useState<TabId>('security');
  const [config, setConfig] = useState<SystemConfig>(DEFAULT_CONFIG);
  const [saved, setSaved] = useState<SystemConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const fetchConfig = useCallback(async () => {
    try {
      setLoading(true);
      const res = await axios.get('/api/config');
      const mergedConfig = {
        ...DEFAULT_CONFIG,
        ...res.data,
        sessionConfig: { ...DEFAULT_CONFIG.sessionConfig, ...(res.data.sessionConfig || {}) },
        lockoutConfig: { ...DEFAULT_CONFIG.lockoutConfig, ...(res.data.lockoutConfig || {}) },
        attendanceConfig: { ...DEFAULT_CONFIG.attendanceConfig, ...(res.data.attendanceConfig || {}) },
        rateLimits: { ...DEFAULT_CONFIG.rateLimits, ...(res.data.rateLimits || {}) },
        gpsValidation: { ...DEFAULT_CONFIG.gpsValidation, ...(res.data.gpsValidation || {}) },
        emulatorDetection: { ...DEFAULT_CONFIG.emulatorDetection, ...(res.data.emulatorDetection || {}) },
        trustScore: { ...DEFAULT_CONFIG.trustScore, ...(res.data.trustScore || {}) },
        webauthnConfig: { ...DEFAULT_CONFIG.webauthnConfig, ...(res.data.webauthnConfig || {}) },
        photoVerification: { ...DEFAULT_CONFIG.photoVerification, ...(res.data.photoVerification || {}) },
      };
      setConfig(mergedConfig);
      setSaved(mergedConfig);
    } catch {
      toast.error('Failed to load system configuration');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetchConfig(); }, [fetchConfig]);

  const activeTabInfo = TABS.find(t => t.id === activeTab)!;
  const hasChanges = JSON.stringify(config) !== JSON.stringify(saved);

  const handleSave = async () => {
    setSaving(true);
    try {
      const res = await axios.put('/api/config', config);
      setSaved(res.data.config ?? config);
      toast.success('Settings saved successfully!');
    } catch (err: unknown) {
      toast.error(axios.isAxiosError(err) ? (err.response?.data?.message || 'Save failed') : 'Save failed');
    } finally {
      setSaving(false);
    }
  };

  const handleReset = () => {
    const sectionDefaults: Partial<SystemConfig> = {
      security: { devBypassEnabled: DEFAULT_CONFIG.devBypassEnabled },
      gps: { gpsValidation: DEFAULT_CONFIG.gpsValidation },
      emulator: { emulatorDetection: DEFAULT_CONFIG.emulatorDetection },
      trust: { trustScore: DEFAULT_CONFIG.trustScore },
      ratelimit: { rateLimits: DEFAULT_CONFIG.rateLimits },
      webauthn: { webauthnConfig: DEFAULT_CONFIG.webauthnConfig },
      photo: { photoVerification: DEFAULT_CONFIG.photoVerification },
    }[activeTab] ?? {};
    setConfig(prev => ({ ...prev, ...sectionDefaults }));
    toast.info(`${activeTabInfo.label} reset to defaults`);
  };

  if (loading) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: 400 }}>
        <div style={{ textAlign: 'center', color: 'var(--color-muted)' }}>
          <div style={{ display: 'flex', justifyContent: 'center', marginBottom: 12 }}>
            <SettingsIcon size={28} className="animate-spin" style={{ color: 'var(--color-primary)' }} />
          </div>
          <p>Loading configuration…</p>
        </div>
      </div>
    );
  }

  return (
    <div className="fade-in settings-container">
      {/* Page Header */}
      <div style={{ marginBottom: 28 }}>
        <h2 style={{ fontSize: 28, fontWeight: 700, color: 'var(--color-text)', margin: '0 0 6px 0' }}>
          System Configuration
        </h2>
        <p style={{ color: 'var(--color-muted)', fontSize: 14, margin: 0 }}>
          All settings are persisted to the database and applied in real-time — no restart required.
        </p>
      </div>

      <AccountSection />

      {/* Two-column layout */}
      <div className="settings-layout">
        {/* Sidebar */}
        <div className="settings-sidebar">
          {TABS.map(tab => {
            const isActive = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className="settings-tab-btn"
                style={{
                  borderLeft: `3px solid ${isActive ? tab.color : 'transparent'}`,
                  background: isActive ? `${tab.color}15` : 'transparent',
                }}
              >
                <span style={{ color: isActive ? tab.color : 'var(--color-muted)', display: 'flex', flexShrink: 0 }}>{tab.icon}</span>
                <span style={{ fontSize: 13, fontWeight: isActive ? 600 : 500, color: isActive ? 'var(--color-text)' : 'var(--color-muted)', flex: 1 }}>{tab.label}</span>
                <span style={{
                  fontSize: 11, padding: '2px 6px', borderRadius: 10,
                  background: isActive ? `${tab.color}25` : 'var(--color-bg-subtle)',
                  color: isActive ? tab.color : 'var(--color-muted)', fontWeight: 600,
                }}>{tab.count}</span>
              </button>
            );
          })}
        </div>

        {/* Main Panel */}
        <div className="settings-panel">
          {/* Panel header */}
          <div className="settings-panel-header" style={{
            background: `linear-gradient(135deg, ${activeTabInfo.color}10, transparent)`,
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
              <div style={{
                width: 40, height: 40, borderRadius: 10, display: 'flex', alignItems: 'center', justifyContent: 'center',
                background: `${activeTabInfo.color}20`, color: activeTabInfo.color,
              }}>
                {activeTabInfo.icon}
              </div>
              <div>
                <div style={{ fontSize: 16, fontWeight: 600, color: 'var(--color-text)' }}>{activeTabInfo.label}</div>
                <div style={{ fontSize: 12, color: 'var(--color-muted)', marginTop: 2 }}>{activeTabInfo.count} configurable settings</div>
              </div>
            </div>
            <button
              onClick={handleReset}
              style={{
                display: 'flex', alignItems: 'center', gap: 6, padding: '7px 14px',
                border: '1px solid var(--color-border)', borderRadius: 8, background: 'var(--color-bg-subtle)',
                color: 'var(--color-muted)', cursor: 'pointer', fontSize: 13, fontWeight: 500,
                transition: 'all 0.15s', whiteSpace: 'nowrap', flexShrink: 0,
              }}
              title={`Reset ${activeTabInfo.label} to defaults`}
            >
              <RotateCcw size={13} /> Reset to Defaults
            </button>
          </div>

          {/* Settings content */}
          <div className="settings-panel-body">
            {activeTab === 'security' && (
              <SecuritySection config={config} onChange={setConfig} />
            )}
            {activeTab === 'gps' && (
              <GpsSection cfg={config.gpsValidation}
                onChange={gpsValidation => setConfig(p => ({ ...p, gpsValidation }))} />
            )}
            {activeTab === 'emulator' && (
              <EmulatorSection cfg={config.emulatorDetection}
                onChange={emulatorDetection => setConfig(p => ({ ...p, emulatorDetection }))} />
            )}
            {activeTab === 'trust' && (
              <TrustSection cfg={config.trustScore}
                onChange={trustScore => setConfig(p => ({ ...p, trustScore }))} />
            )}
            {activeTab === 'ratelimit' && (
              <RateLimitSection cfg={config.rateLimits}
                onChange={rateLimits => setConfig(p => ({ ...p, rateLimits }))} />
            )}
            {activeTab === 'webauthn' && (
              <WebAuthnSection cfg={config.webauthnConfig}
                onChange={webauthnConfig => setConfig(p => ({ ...p, webauthnConfig }))} />
            )}
            {activeTab === 'photo' && (
              <PhotoSection cfg={config.photoVerification}
                onChange={photoVerification => setConfig(p => ({ ...p, photoVerification }))} />
            )}
          </div>

          {/* Sticky footer */}
          <div className="settings-footer">
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              {hasChanges ? (
                <>
                  <span style={{ width: 8, height: 8, borderRadius: '50%', background: '#f97316', display: 'inline-block' }} />
                  <span style={{ fontSize: 13, color: '#f97316', fontWeight: 500 }}>Unsaved changes in {activeTabInfo.label}</span>
                </>
              ) : (
                <>
                  <CheckCircle2 size={14} style={{ color: '#22c55e' }} />
                  <span style={{ fontSize: 13, color: '#22c55e', fontWeight: 500 }}>All settings saved</span>
                </>
              )}
            </div>
            <div className="settings-footer-actions">
              {hasChanges && (
                <button
                  onClick={() => setConfig(saved)}
                  style={{
                    padding: '8px 16px', borderRadius: 8, border: '1px solid var(--color-border)',
                    background: 'transparent', color: 'var(--color-muted)', cursor: 'pointer', fontSize: 13,
                    whiteSpace: 'nowrap', flexShrink: 0,
                  }}
                >Discard</button>
              )}
              <button
                onClick={handleSave}
                disabled={saving || !hasChanges}
                style={{
                  display: 'flex', alignItems: 'center', gap: 7,
                  padding: '8px 20px', borderRadius: 8, border: 'none', cursor: hasChanges ? 'pointer' : 'default',
                  background: hasChanges
                    ? `linear-gradient(135deg, ${activeTabInfo.color}, ${activeTabInfo.color}cc)`
                    : 'var(--color-bg-subtle)',
                  color: hasChanges ? '#fff' : 'var(--color-muted)',
                  fontSize: 13, fontWeight: 600,
                  boxShadow: hasChanges ? `0 0 16px ${activeTabInfo.color}40` : 'none',
                  transition: 'all 0.2s',
                  whiteSpace: 'nowrap', flexShrink: 0,
                }}
              >
                <Save size={14} />
                {saving ? 'Saving…' : `Save ${activeTabInfo.label}`}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Settings;
