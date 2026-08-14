import { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { useNavigate } from 'react-router';
import { toast } from 'react-toastify';
import { MapPin, Clock, Users, ClipboardX, Loader2, Activity, CalendarClock, GraduationCap, PieChart } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useAuth } from '../context/AuthContext';

interface Session {
  _id: string;
  locationName?: string;
  batchName?: string;
  collegeName?: string;
  startsAt?: string;
  expiresAt: string;
  isActive: boolean;
  attendanceCount: number;
  description?: string;
}

interface RosterSummary {
  total: number;
  marked: number;
  present: number;
  absent: number;
  unmarked: number;
}

type SessionState = 'live' | 'upcoming' | 'closed';

const sessionState = (s: Session): SessionState => {
  const now = Date.now();
  if (!s.isActive || new Date(s.expiresAt).getTime() < now) return 'closed';
  if (s.startsAt && new Date(s.startsAt).getTime() > now) return 'upcoming';
  return 'live';
};

const formatSlot = (s: Session) => {
  if (!s.startsAt) return null;
  return new Date(s.startsAt).toLocaleString(undefined, {
    weekday: 'short', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
  });
};

const timeUntil = (iso: string) => {
  const diffMin = Math.round((new Date(iso).getTime() - Date.now()) / 60000);
  if (diffMin <= 0) return 'Starting now';
  const h = Math.floor(diffMin / 60);
  const m = diffMin % 60;
  return h > 0 ? `Starts in ${h}h ${m}m` : `Starts in ${m}m`;
};

const greetingWord = () => {
  const hour = new Date().getHours();
  if (hour < 12) return 'Good morning';
  if (hour < 17) return 'Good afternoon';
  return 'Good evening';
};

const Dashboard = () => {
  const navigate = useNavigate();
  const { admin } = useAuth();
  const displayName = admin?.fullName || admin?.username || '';
  const [sessions, setSessions] = useState<Session[]>([]);
  const [rosterMap, setRosterMap] = useState<Record<string, RosterSummary>>({});
  const [loading, setLoading] = useState(true);

  const fetchSessions = useCallback(async () => {
    try {
      const res = await axios.get<Session[]>('/api/admin/sessions');
      setSessions(res.data);

      const open = res.data.filter((s) => sessionState(s) !== 'closed');
      const entries = await Promise.all(open.map(async (s) => {
        try {
          const r = await axios.get<{ summary: RosterSummary }>(`/api/admin/sessions/${s._id}/roster`);
          return [s._id, r.data.summary] as const;
        } catch {
          return [s._id, null] as const;
        }
      }));
      setRosterMap(Object.fromEntries(entries.filter((e): e is [string, RosterSummary] => e[1] !== null)));
    } catch {
      toast.error('Failed to load your sessions');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSessions();
    const interval = setInterval(fetchSessions, 30000);
    return () => clearInterval(interval);
  }, [fetchSessions]);

  if (loading) {
    return (
      <div className="loading-screen" style={{ minHeight: '50vh' }}>
        <Loader2 size={22} className="spin" />
      </div>
    );
  }

  const openSessions = sessions.filter((s) => sessionState(s) !== 'closed');
  const liveSessions = openSessions.filter((s) => sessionState(s) === 'live');
  const upcomingSessions = openSessions.filter((s) => sessionState(s) === 'upcoming');
  const closedSessions = sessions.filter((s) => sessionState(s) === 'closed');
  const scheduled = openSessions
    .filter((s) => s.startsAt)
    .sort((a, b) => new Date(a.startsAt!).getTime() - new Date(b.startsAt!).getTime());

  const rosterTotals = openSessions.reduce(
    (acc, s) => {
      const r = rosterMap[s._id];
      if (r) { acc.total += r.total; acc.present += r.present; acc.marked += r.marked; }
      return acc;
    },
    { total: 0, present: 0, marked: 0 }
  );
  const attendancePct = rosterTotals.marked === 0 ? null : Math.round((rosterTotals.present / rosterTotals.marked) * 100);

  return (
    <div className="fade-in">
      <div style={{ marginBottom: 4 }}>
        <h1 style={{ fontSize: 22, fontWeight: 800, margin: 0 }}>
          {greetingWord()}{displayName ? `, ${displayName}` : ''} 👋
        </h1>
        <p style={{ fontSize: 13, color: 'var(--color-muted)', margin: '4px 0 0' }}>Here&apos;s what&apos;s happening today</p>
      </div>

      {sessions.length === 0 ? (
        <div className="card empty-state" style={{ marginTop: 20 }}>
          <ClipboardX size={32} style={{ margin: '0 auto 12px', color: 'var(--color-faint)' }} />
          <p>No sessions assigned to you yet.</p>
          <p style={{ fontSize: 13 }}>Ask your super admin to assign you to a session.</p>
        </div>
      ) : (
        <>
          <div className="stat-grid">
            <StatCard icon={Activity} label="Active Sessions" value={liveSessions.length} tone="primary" />
            <StatCard icon={CalendarClock} label="Upcoming" value={upcomingSessions.length} tone="warning" />
            <StatCard icon={GraduationCap} label="Students" value={rosterTotals.total} tone="success" />
            <StatCard icon={PieChart} label="Attendance" value={attendancePct === null ? '—' : `${attendancePct}%`} tone="primary" />
          </div>

          {openSessions.length > 0 && (
            <div className="panel">
              <div className="panel-header">Your Sessions Today</div>
              <div>
                {openSessions.map((s) => {
                  const state = sessionState(s);
                  const roster = rosterMap[s._id];
                  return (
                    <div key={s._id} className="session-row" onClick={() => navigate(`/sessions/${s._id}`)}>
                      <div>
                        <div className="session-row-title">{s.batchName || s.collegeName || 'Session'}</div>
                        <div className="session-row-sub">
                          {[s.collegeName, s.locationName].filter(Boolean).join(' · ') || 'No location'}
                        </div>
                      </div>
                      <div className="session-row-meta">
                        <span className={`badge ${state === 'live' ? 'badge-success' : 'badge-warning'}`}>
                          {state === 'live' ? 'LIVE' : (s.startsAt ? timeUntil(s.startsAt) : 'Upcoming')}
                        </span>
                        <span className="session-row-count">
                          {roster ? `${roster.marked}/${roster.total}` : `${s.attendanceCount}`} Marked
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {scheduled.length > 0 && (
            <div className="panel">
              <div className="panel-header">Today&apos;s Schedule</div>
              <div className="timeline">
                {scheduled.map((s) => {
                  const state = sessionState(s);
                  return (
                    <div key={s._id} className="timeline-item">
                      <div className="timeline-marker">
                        <span className={`timeline-dot${state === 'live' ? ' live' : ''}`} />
                      </div>
                      <div>
                        <div className="timeline-time">
                          {new Date(s.startsAt!).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })}
                          {state === 'upcoming' && ' · Upcoming'}
                        </div>
                        <div className="timeline-title">{s.batchName || s.collegeName || 'Session'}</div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {rosterTotals.marked > 0 && (
            <div className="panel">
              <div className="panel-header">Attendance Overview</div>
              <div className="donut-wrap">
                <div
                  className="donut"
                  style={{ background: `conic-gradient(var(--color-success) 0% ${attendancePct}%, var(--color-danger) ${attendancePct}% 100%)` }}
                >
                  <div className="donut-center">
                    <div className="donut-pct">{attendancePct}%</div>
                    <div className="donut-label">Present</div>
                  </div>
                </div>
                <div className="donut-legend">
                  <div><span className="legend-dot present" /> Present <strong>{rosterTotals.present}</strong></div>
                  <div><span className="legend-dot absent" /> Absent <strong>{rosterTotals.marked - rosterTotals.present}</strong></div>
                </div>
              </div>
            </div>
          )}

          {closedSessions.length > 0 && (
            <>
              <div className="section-heading">Past / Inactive</div>
              {closedSessions.map((s) => <SessionCard key={s._id} session={s} muted onClick={() => navigate(`/sessions/${s._id}`)} />)}
            </>
          )}
        </>
      )}
    </div>
  );
};

const StatCard = ({ icon: Icon, label, value, tone }: { icon: LucideIcon; label: string; value: number | string; tone: 'primary' | 'success' | 'warning' }) => (
  <div className="stat-card">
    <div className={`stat-card-icon tone-${tone}`}><Icon size={17} /></div>
    <div>
      <div className="stat-card-value">{value}</div>
      <div className="stat-card-label">{label}</div>
    </div>
  </div>
);

const SessionCard = ({ session, onClick, muted }: { session: Session; onClick: () => void; muted?: boolean }) => {
  const slot = formatSlot(session);
  return (
    <div className="card session-card" onClick={onClick} style={{ opacity: muted ? 0.65 : 1 }}>
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 8 }}>
        <div>
          <div style={{ fontSize: 17, fontWeight: 700 }}>{session.collegeName || session.locationName || 'Session'}</div>
          {session.batchName && <div style={{ fontSize: 13, color: 'var(--color-muted)', marginTop: 2 }}>{session.batchName}</div>}
        </div>
        <span className="badge badge-neutral">Closed</span>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '10px 16px', marginTop: 14, fontSize: 13, color: 'var(--color-muted)' }}>
        {slot && <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Clock size={14} /> {slot}</span>}
        {session.locationName && <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><MapPin size={14} /> {session.locationName}</span>}
        <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Users size={14} /> {session.attendanceCount} marked</span>
      </div>
    </div>
  );
};

export default Dashboard;
