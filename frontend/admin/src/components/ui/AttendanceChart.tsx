import { useEffect, useMemo, useState, useCallback } from 'react';
import axios from 'axios';
import { Calendar } from 'lucide-react';
import Modal from './Modal';

/**
 * Dashboard attendance visualization — hand-built SVG bar chart (no chart dep).
 *
 * Two modes:
 *  • Overview (default) — one bar per day/week/month = total students who filled
 *    attendance, consolidated across all sessions.
 *  • By date — pick a date, choose that day's sessions, chart per-session counts.
 *
 * Live pulls the real endpoints; Demo uses a generated dataset so the chart is
 * populated before any real attendance exists. Demo data lives only here.
 */

const PALETTE = ['var(--viz-1)', 'var(--viz-2)', 'var(--viz-3)', 'var(--viz-4)'];
const OTHER_COLOR = 'var(--color-faint)';
const MAX_LOC_CATS = 4;

interface Row { date: string; location: string; session: string; time?: string; count: number; }
type Source = 'live' | 'demo';
type Mode = 'overview' | 'date';
type Granularity = 'daily' | 'weekly' | 'monthly';
type GroupBy = 'session' | 'location';

const PERIOD_COUNT: Record<Granularity, number> = { daily: 14, weekly: 8, monthly: 6 };
const DEMO_LOCATIONS = ['ECE Hall', 'DSCS Block', 'Library Wing B'];
const DEMO_DESCRIPTIONS = ['Data Structures', 'Physics Lab', 'ML Tutorial', 'Guest Seminar', 'Networks', 'DBMS Lab'];

function seeded(seed: number) {
  let s = seed || 1;
  return () => { s = (s * 1103515245 + 12345) & 0x7fffffff; return s / 0x7fffffff; };
}

// One dummy dataset drives both modes: per-day, per-session rows over ~120 days.
function generateDemoDataset(): Row[] {
  const rand = seeded(42);
  const out: Row[] = [];
  const today = new Date();
  for (let d = 0; d < 120; d++) {
    const date = new Date(today);
    date.setDate(today.getDate() - d);
    const iso = date.toISOString().slice(0, 10);
    const weekend = date.getDay() === 0 || date.getDay() === 6;
    const nSessions = weekend ? 1 + Math.floor(rand() * 2) : 2 + Math.floor(rand() * 4);
    for (let i = 0; i < nSessions; i++) {
      const location = DEMO_LOCATIONS[Math.floor(rand() * DEMO_LOCATIONS.length)];
      const session = DEMO_DESCRIPTIONS[Math.floor(rand() * DEMO_DESCRIPTIONS.length)];
      const hour = 8 + Math.floor(rand() * 10);
      out.push({ date: iso, location, session, time: `${iso}T${String(hour).padStart(2, '0')}:00:00`, count: 12 + Math.floor(rand() * 42) });
    }
  }
  return out;
}

function parseDay(s: string): Date { const [y, m, d] = s.split('-').map(Number); return new Date(y, m - 1, d); }
function todayISO(): string { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`; }
function fmtDate(iso: string): string { const [y, m, d] = iso.split('-').map(Number); return new Date(y, m - 1, d).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' }); }
function fmtTime(t?: string): string { return t ? new Date(t).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : ''; }

function periodKey(date: Date, g: Granularity): { key: string; label: string; sort: number } {
  if (g === 'monthly') return { key: `${date.getFullYear()}-${date.getMonth()}`, label: date.toLocaleDateString(undefined, { month: 'short' }), sort: date.getFullYear() * 12 + date.getMonth() };
  if (g === 'weekly') { const s = new Date(date); s.setDate(date.getDate() - date.getDay()); s.setHours(0, 0, 0, 0); return { key: s.toISOString().slice(0, 10), label: s.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }), sort: s.getTime() }; }
  return { key: date.toISOString().slice(0, 10), label: date.toLocaleDateString(undefined, { day: 'numeric' }), sort: date.getTime() };
}

function roundedTopPath(x: number, y: number, w: number, h: number, r: number): string {
  if (h <= 0) return '';
  const rr = Math.min(r, w / 2, h);
  return `M${x},${y + h} L${x},${y + rr} Q${x},${y} ${x + rr},${y} L${x + w - rr},${y} Q${x + w},${y} ${x + w},${y + rr} L${x + w},${y + h} Z`;
}

interface Bar { label: string; sub?: string; value: number; color: string; }

const AttendanceChart = () => {
  const [source, setSource] = useState<Source>('demo');
  const [mode, setMode] = useState<Mode>('overview');
  const [granularity, setGranularity] = useState<Granularity>('daily');
  const [locationFilter, setLocationFilter] = useState('all');
  const [groupBy, setGroupBy] = useState<GroupBy>('session');
  const [hover, setHover] = useState<number | null>(null);

  const [liveSeries, setLiveSeries] = useState<Row[]>([]);
  const [liveLocations, setLiveLocations] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // date-drill state
  const [modalOpen, setModalOpen] = useState(false);
  const [date, setDate] = useState(todayISO());
  const [sessions, setSessions] = useState<Row[]>([]);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [loadingSessions, setLoadingSessions] = useState(false);
  const [applied, setApplied] = useState<{ date: string; sessions: Row[] } | null>(null);

  const demoData = useMemo(generateDemoDataset, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [seriesRes, locRes] = await Promise.all([
          axios.get<Row[]>('/api/admin/dashboard/attendance-series'),
          axios.get<{ name: string }[]>('/api/admin/locations'),
        ]);
        if (cancelled) return;
        setLiveSeries(seriesRes.data);
        setLiveLocations([...new Set(locRes.data.map((l) => l.name))]);
        setError(null);
      } catch { if (!cancelled) setError('Failed to load attendance data.'); } finally { if (!cancelled) setLoading(false); }
    })();
    return () => { cancelled = true; };
  }, []);

  const rows = source === 'demo' ? demoData : liveSeries;
  const allLocations = source === 'demo' ? DEMO_LOCATIONS : liveLocations;

  // ── Overview: consolidated per-period totals ──────────────
  const overviewBars: Bar[] = useMemo(() => {
    const filtered = locationFilter === 'all' ? rows : rows.filter((r) => r.location === locationFilter);
    const buckets = new Map<string, { label: string; sort: number; total: number }>();
    for (const r of filtered) {
      const { key, label, sort } = periodKey(parseDay(r.date), granularity);
      const b = buckets.get(key) ?? { label, sort, total: 0 };
      b.total += r.count;
      buckets.set(key, b);
    }
    return [...buckets.values()].sort((a, b) => a.sort - b.sort).slice(-PERIOD_COUNT[granularity])
      .map((b) => ({ label: b.label, value: b.total, color: 'var(--color-primary)' }));
  }, [rows, locationFilter, granularity]);

  // ── Date mode: per-session bars for a chosen date ─────────
  const appliedLocations = useMemo(() => [...new Set((applied?.sessions ?? []).map((s) => s.location))], [applied]);
  const colorForLoc = (loc: string) => { const i = appliedLocations.indexOf(loc); return i >= 0 && i < MAX_LOC_CATS ? PALETTE[i] : OTHER_COLOR; };

  const dateBars: Bar[] = useMemo(() => {
    if (!applied) return [];
    const rws = locationFilter === 'all' ? applied.sessions : applied.sessions.filter((s) => s.location === locationFilter);
    if (groupBy === 'location') {
      const m = new Map<string, number>();
      for (const s of rws) m.set(s.location, (m.get(s.location) ?? 0) + s.count);
      return [...m.entries()].map(([location, value]) => ({ label: location, value, color: colorForLoc(location) }));
    }
    return rws.map((s) => ({ label: s.session, sub: `${s.location}${s.time ? ' · ' + fmtTime(s.time) : ''}`, value: s.count, color: colorForLoc(s.location) }));
  }, [applied, locationFilter, groupBy, appliedLocations]);

  const bars = mode === 'overview' ? overviewBars : dateBars;

  // ── Date modal loading ────────────────────────────────────
  const loadSessions = useCallback(async (d: string) => {
    if (source === 'demo') {
      const gen = demoData.filter((r) => r.date === d).sort((a, b) => (a.time || '').localeCompare(b.time || ''))
        .map((r, i) => ({ ...r, sessionId: `${d}-${i}` } as Row & { sessionId: string }));
      setSessions(gen); setChecked(new Set(gen.map((s) => (s as Row & { sessionId: string }).sessionId)));
      return;
    }
    setLoadingSessions(true);
    try {
      const res = await axios.get<(Row & { sessionId: string })[]>('/api/admin/dashboard/sessions-by-date', { params: { date: d } });
      setSessions(res.data); setChecked(new Set(res.data.map((s) => s.sessionId)));
    } catch { setSessions([]); setChecked(new Set()); } finally { setLoadingSessions(false); }
  }, [source, demoData]);

  const sid = (s: Row) => (s as Row & { sessionId?: string }).sessionId ?? `${s.date}-${s.session}-${s.time}`;
  const openModal = useCallback(() => { setModalOpen(true); loadSessions(date); }, [date, loadSessions]);
  const onDateChange = (d: string) => { setDate(d); loadSessions(d); };
  const toggleCheck = (id: string) => setChecked((p) => { const n = new Set(p); if (n.has(id)) { n.delete(id); } else { n.add(id); } return n; });
  const checkNow = () => { setApplied({ date, sessions: sessions.filter((s) => checked.has(sid(s))) }); setLocationFilter('all'); setMode('date'); setModalOpen(false); };
  const switchSource = useCallback((s: Source) => { setSource(s); setApplied(null); setLocationFilter('all'); }, []);
  const switchMode = useCallback((m: Mode) => { setMode(m); setLocationFilter('all'); if (m === 'date' && !applied) openModal(); }, [applied, openModal]);

  // ── Geometry ──────────────────────────────────────────────
  const VBW = 760, VBH = 300;
  const pad = { top: 28, right: 16, bottom: 46, left: 44 };
  const plotW = VBW - pad.left - pad.right;
  const plotH = VBH - pad.top - pad.bottom;
  const rawMax = Math.max(1, ...bars.map((b) => b.value));
  const stepMag = Math.pow(10, Math.floor(Math.log10(rawMax)));
  const yMax = Math.ceil(rawMax / stepMag) * stepMag || 1;
  const yTicks = [0, 0.25, 0.5, 0.75, 1].map((f) => Math.round(yMax * f));
  const n = bars.length;
  const bandW = plotW / Math.max(n, 1);
  const barW = Math.min(bandW * 0.58, 60);
  const yScale = (v: number) => plotH - (v / yMax) * plotH;
  const truncate = (s: string, len: number) => (s.length > len ? s.slice(0, len - 1) + '…' : s);

  const locList = mode === 'overview' ? allLocations : appliedLocations;
  const subtitle = mode === 'overview'
    ? `Students who filled attendance per ${granularity === 'daily' ? 'day' : granularity === 'weekly' ? 'week' : 'month'}`
    : applied ? `${fmtDate(applied.date)} · ${applied.sessions.length} session${applied.sessions.length !== 1 ? 's' : ''}` : 'Pick a date';

  return (
    <div className="chart-card">
      <div className="chart-header">
        <div>
          <div className="chart-title">Attendance Overview</div>
          <div className="chart-subtitle">{subtitle}{source === 'demo' && <span className="chart-demo-tag">demo data</span>}</div>
        </div>
        <div className="chart-controls">
          <div className="seg" role="radiogroup">
            {(['live', 'demo'] as Source[]).map((s) => <button key={s} role="radio" aria-checked={source === s} className={source === s ? 'active' : ''} onClick={() => switchSource(s)}>{s === 'live' ? 'Live' : 'Demo'}</button>)}
          </div>
          <div className="seg" role="radiogroup">
            {(['overview', 'date'] as Mode[]).map((m) => <button key={m} role="radio" aria-checked={mode === m} className={mode === m ? 'active' : ''} onClick={() => switchMode(m)}>{m === 'overview' ? 'Overview' : 'By date'}</button>)}
          </div>
          {mode === 'date' && <button className="btn btn-secondary btn-small" onClick={openModal}><Calendar size={14} /> {applied ? fmtDate(applied.date) : 'Pick date'}</button>}
          {(mode === 'overview' || applied) && (
            <select className="chart-select" value={locationFilter} onChange={(e) => setLocationFilter(e.target.value)}>
              <option value="all">All Locations</option>
              {locList.map((l) => <option key={l} value={l}>{l}</option>)}
            </select>
          )}
          {mode === 'overview' && (
            <div className="seg">
              {(['daily', 'weekly', 'monthly'] as Granularity[]).map((g) => <button key={g} className={granularity === g ? 'active' : ''} onClick={() => setGranularity(g)}>{g[0].toUpperCase() + g.slice(1)}</button>)}
            </div>
          )}
          {mode === 'date' && applied && (
            <div className="seg">
              {(['session', 'location'] as GroupBy[]).map((g) => <button key={g} className={groupBy === g ? 'active' : ''} onClick={() => setGroupBy(g)}>{g === 'session' ? 'By session' : 'By location'}</button>)}
            </div>
          )}
        </div>
      </div>

      {error ? (
        <div className="chart-empty chart-error">{error}</div>
      ) : source === 'live' && loading ? (
        <div className="chart-empty">Loading attendance…</div>
      ) : n === 0 ? (
        <div className="chart-empty">{mode === 'date' && !applied ? 'Pick a date to load its sessions.' : 'No attendance recorded yet.'}</div>
      ) : (
        <>
          <div className="chart-plot" onMouseLeave={() => setHover(null)}>
            <svg viewBox={`0 0 ${VBW} ${VBH}`} style={{ width: '100%', height: 'auto' }} role="img" aria-label="Attendance bar chart">
              <defs>
                <filter id="barShadow" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="2" stdDeviation="3" floodOpacity="0.16" /></filter>
              </defs>
              {yTicks.map((t) => { const y = pad.top + yScale(t); return (
                <g key={t}>
                  <line className="chart-grid-line" x1={pad.left} y1={y} x2={pad.left + plotW} y2={y} />
                  <text className="chart-tick" x={pad.left - 8} y={y + 4} textAnchor="end">{t}</text>
                </g>); })}
              {bars.map((b, i) => {
                const bx = pad.left + i * bandW + (bandW - barW) / 2;
                const yTop = pad.top + yScale(b.value);
                const h = plotH - yScale(b.value);
                return (
                  <g key={i} className="chart-bar-group" onMouseEnter={() => setHover(i)}>
                    <rect x={pad.left + i * bandW} y={pad.top} width={bandW} height={plotH} fill="transparent" />
                    <path d={roundedTopPath(bx, pad.top + 2, barW, plotH - 2, 5)} className="chart-bar-track" />
                    <path className="chart-bar-seg" d={roundedTopPath(bx, yTop, barW, h, 5)} fill={b.color} filter="url(#barShadow)" />
                    <text className="chart-bar-value" x={bx + barW / 2} y={yTop - 8} textAnchor="middle">{b.value}</text>
                    <text className="chart-axis-label" x={bx + barW / 2} y={VBH - pad.bottom + 18} textAnchor="middle">{truncate(b.label, 12)}</text>
                  </g>
                );
              })}
            </svg>
            {hover !== null && bars[hover] && (
              <div className="chart-tooltip" style={{ left: `${((pad.left + hover * bandW + bandW / 2) / VBW) * 100}%`, top: `${((pad.top + yScale(bars[hover].value) - 12) / VBH) * 100}%` }}>
                <div className="chart-tooltip-title">{bars[hover].label}</div>
                {bars[hover].sub && <div className="chart-tooltip-row" style={{ marginBottom: 4 }}>{bars[hover].sub}</div>}
                <div className="chart-tooltip-row"><span className="chart-tooltip-dot" style={{ background: bars[hover].color }} />{bars[hover].value} students</div>
              </div>
            )}
          </div>
          {mode === 'date' && appliedLocations.length > 1 && (
            <div className="chart-legend">
              {appliedLocations.map((l) => <span key={l} className="chart-legend-item"><span className="chart-legend-swatch" style={{ background: colorForLoc(l) }} />{l}</span>)}
            </div>
          )}
        </>
      )}

      <Modal open={modalOpen} onClose={() => setModalOpen(false)} title="Select date & sessions"
        footer={<>
          <button className="btn btn-primary" onClick={checkNow} disabled={checked.size === 0}>Check now</button>
          <button className="btn btn-secondary" onClick={() => setModalOpen(false)}>Cancel</button>
        </>}>
        <div className="form-group">
          <label>Date</label>
          <input type="date" value={date} max={todayISO()} onChange={(e) => onDateChange(e.target.value)} />
        </div>
        <div className="form-group" style={{ marginBottom: 0 }}>
          <label>Sessions on this date {sessions.length > 0 && `(${checked.size}/${sessions.length} selected)`}</label>
          {loadingSessions ? <div className="session-pick-empty">Loading sessions…</div>
            : sessions.length === 0 ? <div className="session-pick-empty">No sessions recorded on this date.</div>
            : <div className="session-pick-list">
                {sessions.map((s) => (
                  <label key={sid(s)} className="session-pick-item">
                    <input type="checkbox" checked={checked.has(sid(s))} onChange={() => toggleCheck(sid(s))} />
                    <span className="session-pick-body">
                      <span className="session-pick-name">{s.session}</span>
                      <span className="session-pick-meta">{s.location}{s.time ? ' · ' + fmtTime(s.time) : ''}</span>
                    </span>
                    <span className="session-pick-count">{s.count}</span>
                  </label>
                ))}
              </div>}
        </div>
      </Modal>
    </div>
  );
};

export default AttendanceChart;
