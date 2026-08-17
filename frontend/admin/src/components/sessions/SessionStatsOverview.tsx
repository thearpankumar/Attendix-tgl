import { Link } from 'react-router';
import { Calendar, CheckCircle, Percent, TrendingDown, TrendingUp, Users, XCircle } from 'lucide-react';

export interface SessionHighlight {
  id: string;
  label: string;
  attendancePercent: number;
}

export interface SessionRecordsOverview {
  totalRecords: number;
  filteredRecords: number;
  present: number;
  absent: number;
  attendancePercent: number;
  topSession: SessionHighlight | null;
  bottomSession: SessionHighlight | null;
}

interface Props {
  overview: SessionRecordsOverview | null;
}

// Global counts for the currently active filters — scanned server-side with
// no page-size cap, so these stay accurate even once the sessions table
// itself is capped for display (see SESSIONS_LIST_PAGE_SIZE on the backend).
const SessionStatsOverview = ({ overview }: Props) => {
  const showHighlights = !!(overview?.topSession || overview?.bottomSession);
  const sameSession = overview?.topSession?.id === overview?.bottomSession?.id;

  return (
    <>
      <div className="session-records-stats">
        <div className="stat-card-new stat-card-total">
          <div className="stat-card-icon-wrapper"><Calendar size={28} /></div>
          <span className="stat-card-label">TOTAL DB RECORDS</span>
          <span className="stat-card-value">{overview?.totalRecords ?? 0}</span>
          <div className="stat-card-accent-line"></div>
        </div>

        <div className="stat-card-new stat-card-filtered">
          <div className="stat-card-icon-wrapper"><Users size={28} /></div>
          <span className="stat-card-label">FILTERED RECORDS</span>
          <span className="stat-card-value">{overview?.filteredRecords ?? 0}</span>
          <div className="stat-card-accent-line"></div>
        </div>

        <div className="stat-card-new stat-card-verified">
          <div className="stat-card-icon-wrapper"><CheckCircle size={28} /></div>
          <span className="stat-card-label">PRESENT</span>
          <span className="stat-card-value">{overview?.present ?? 0}</span>
          <div className="stat-card-accent-line"></div>
        </div>

        <div className="stat-card-new stat-card-unverified">
          <div className="stat-card-icon-wrapper"><XCircle size={28} /></div>
          <span className="stat-card-label">ABSENT</span>
          <span className="stat-card-value">{overview?.absent ?? 0}</span>
          <div className="stat-card-accent-line"></div>
        </div>

        <div className="stat-card-new stat-card-percent">
          <div className="stat-card-icon-wrapper"><Percent size={28} /></div>
          <span className="stat-card-label">ATTENDANCE</span>
          <span className="stat-card-value">{Math.round(overview?.attendancePercent ?? 0)}%</span>
          <div className="stat-card-accent-line"></div>
        </div>
      </div>

      {showHighlights && (
        <div className="session-highlights-row">
          {overview?.topSession && (
            <Link to={`/sessions/${overview.topSession.id}`} className="session-highlight-chip session-highlight-best">
              <TrendingUp size={14} />
              Best attendance: {overview.topSession.label} — {Math.round(overview.topSession.attendancePercent)}%
            </Link>
          )}
          {overview?.bottomSession && !sameSession && (
            <Link to={`/sessions/${overview.bottomSession.id}`} className="session-highlight-chip session-highlight-worst">
              <TrendingDown size={14} />
              Needs attention: {overview.bottomSession.label} — {Math.round(overview.bottomSession.attendancePercent)}%
            </Link>
          )}
        </div>
      )}
    </>
  );
};

export default SessionStatsOverview;
