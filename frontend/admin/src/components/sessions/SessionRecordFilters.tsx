import { useEffect, useMemo, useState } from 'react';
import { Download } from 'lucide-react';
import MultiSelect from '../ui/MultiSelect';
import Button from '../ui/Button';

export interface SessionRecordFiltersState {
  batchIds: string[];
  classLabels: string[];
  tracks: string[];
  timeSlots: string[];
  markingStatus: string;
  search: string;
}

export const emptyRecordFilters: SessionRecordFiltersState = {
  batchIds: [],
  classLabels: [],
  tracks: [],
  timeSlots: [],
  markingStatus: '',
  search: '',
};

interface RecordSession {
  locationName?: string;
  track?: string;
  sessionTimeRaw?: string;
  /** Manual-batch sessions only. */
  batchId?: string;
  /** Excel-upload (mentor) sessions only. */
  excelBatchId?: string;
}

interface BatchOption {
  _id: string;
  name: string;
  type?: 'manual' | 'session';
}

interface Props {
  allSessions: RecordSession[];
  batches: BatchOption[];
  filters: SessionRecordFiltersState;
  onChange: (next: SessionRecordFiltersState) => void;
  onExportFiltered: () => void;
  exporting: boolean;
  exportCount: number;
}

const MARKING_STATUS_OPTIONS = [
  { value: '', label: 'All Status' },
  { value: 'not_started', label: 'Not Started' },
  { value: 'partial', label: 'Partially Marked' },
  { value: 'complete', label: 'Fully Marked' },
];

type FacetKey = 'batchIds' | 'classLabels' | 'tracks' | 'timeSlots';

const sessionBatchKey = (s: RecordSession): string | undefined => s.batchId ?? s.excelBatchId;

/** Whether `s` satisfies every active facet filter *except* `exclude` — the
 * standard faceted-search trick that makes each dropdown's own options
 * reflect "what's still possible given your other selections" without ever
 * filtering against itself (which would make a facet unable to offer
 * anything beyond what's already chosen). */
const matchesOtherFacets = (s: RecordSession, filters: SessionRecordFiltersState, exclude: FacetKey): boolean => {
  if (exclude !== 'batchIds' && filters.batchIds.length > 0) {
    const key = sessionBatchKey(s);
    if (!key || !filters.batchIds.includes(key)) return false;
  }
  if (exclude !== 'classLabels' && filters.classLabels.length > 0) {
    if (!s.locationName || !filters.classLabels.includes(s.locationName)) return false;
  }
  if (exclude !== 'tracks' && filters.tracks.length > 0) {
    if (!s.track || !filters.tracks.includes(s.track)) return false;
  }
  if (exclude !== 'timeSlots' && filters.timeSlots.length > 0) {
    if (!s.sessionTimeRaw || !filters.timeSlots.includes(s.sessionTimeRaw)) return false;
  }
  return true;
};

// Batches, Classes, Tracks, and Time Slots all cascade off each other in
// both directions: picking a batch narrows which classes/tracks/time-slots
// are offered (and vice versa for any of the other three), using
// matchesOtherFacets so a facet is never filtered against its own
// selection. Status and Search don't participate in this — they narrow the
// session list itself, not what these four dropdowns offer.
const SessionRecordFilters = ({
  allSessions,
  batches,
  filters,
  onChange,
  onExportFiltered,
  exporting,
  exportCount,
}: Props) => {
  const classOptions = useMemo(() => {
    const set = new Set<string>();
    allSessions
      .filter((s) => matchesOtherFacets(s, filters, 'classLabels'))
      .forEach((s) => { if (s.locationName) set.add(s.locationName); });
    return [...set].sort().map((c) => ({ value: c, label: c }));
  }, [allSessions, filters]);

  const trackOptions = useMemo(() => {
    const set = new Set<string>();
    allSessions
      .filter((s) => matchesOtherFacets(s, filters, 'tracks'))
      .forEach((s) => { if (s.track) set.add(s.track); });
    return [...set].sort().map((t) => ({ value: t, label: t }));
  }, [allSessions, filters]);

  const timeSlotOptions = useMemo(() => {
    const set = new Set<string>();
    allSessions
      .filter((s) => matchesOtherFacets(s, filters, 'timeSlots'))
      .forEach((s) => { if (s.sessionTimeRaw) set.add(s.sessionTimeRaw); });
    return [...set].sort().map((t) => ({ value: t, label: t }));
  }, [allSessions, filters]);

  const batchOptions = useMemo(() => {
    const label = (b: BatchOption) => (b.type === 'session' ? `${b.name} (Session Batch)` : b.name);
    // Only narrow the batch list once one of the other three facets is
    // active — with nothing else selected, every batch is a valid choice
    // even ones with no sessions loaded yet (which wouldn't otherwise
    // appear, since this list is otherwise derived from `allSessions`).
    const facetActive = filters.classLabels.length > 0 || filters.tracks.length > 0 || filters.timeSlots.length > 0;
    if (!facetActive) {
      return batches.map((b) => ({ value: b._id, label: label(b) }));
    }
    const allowedKeys = new Set(
      allSessions
        .filter((s) => matchesOtherFacets(s, filters, 'batchIds'))
        .map(sessionBatchKey)
        .filter((k): k is string => !!k)
    );
    return batches.filter((b) => allowedKeys.has(b._id)).map((b) => ({ value: b._id, label: label(b) }));
  }, [allSessions, batches, filters]);

  const set = <K extends keyof SessionRecordFiltersState>(key: K, value: SessionRecordFiltersState[K]) =>
    onChange({ ...filters, [key]: value });

  // Debounced independently of the other filters (which are click-driven
  // and should apply immediately) so typing a name doesn't fire a request
  // per keystroke.
  const [searchInput, setSearchInput] = useState(filters.search);
  useEffect(() => setSearchInput(filters.search), [filters.search]);
  useEffect(() => {
    if (searchInput === filters.search) return;
    const timeout = setTimeout(() => set('search', searchInput), 400);
    return () => clearTimeout(timeout);
  }, [searchInput]);

  return (
    <div className="card session-record-filters">
      <div className="session-record-filters-header">
        <h3>Filters</h3>
        <Button variant="secondary" size="sm" onClick={onExportFiltered} disabled={exporting || exportCount === 0}>
          <Download size={14} style={{ marginRight: 6, verticalAlign: -2 }} />
          {exporting ? 'Exporting…' : `Export Filtered (${exportCount})`}
        </Button>
      </div>

      <div className="form-group">
        <div className="session-record-filters-label-row">
          <label htmlFor="record-filter-batches">Select Batches (multiple allowed)</label>
          {filters.batchIds.length > 0 && (
            <button type="button" className="link-button" onClick={() => set('batchIds', [])}>Deselect All</button>
          )}
        </div>
        <MultiSelect
          id="record-filter-batches"
          options={batchOptions}
          selected={filters.batchIds}
          onChange={(next) => set('batchIds', next)}
          placeholder={batchOptions.length === 0 ? 'No batches available for the selected filters' : 'Search batches…'}
          emptyMessage="No batches available for the selected filters"
        />
      </div>

      <div className="session-record-filters-row">
        <div className="form-group">
          <div className="session-record-filters-label-row">
            <label htmlFor="record-filter-classes">Select Classes</label>
            {filters.classLabels.length > 0 && (
              <button type="button" className="link-button" onClick={() => set('classLabels', [])}>Deselect All</button>
            )}
          </div>
          <MultiSelect
            id="record-filter-classes"
            options={classOptions}
            selected={filters.classLabels}
            onChange={(next) => set('classLabels', next)}
            placeholder="Search classes…"
            emptyMessage="No classes available for the selected filters"
          />
        </div>

        <div className="form-group">
          <div className="session-record-filters-label-row">
            <label htmlFor="record-filter-tracks">Select Tracks</label>
            {filters.tracks.length > 0 && (
              <button type="button" className="link-button" onClick={() => set('tracks', [])}>Deselect All</button>
            )}
          </div>
          <MultiSelect
            id="record-filter-tracks"
            options={trackOptions}
            selected={filters.tracks}
            onChange={(next) => set('tracks', next)}
            placeholder="Search tracks…"
            emptyMessage="No tracks available for the selected filters"
          />
        </div>

        <div className="form-group">
          <div className="session-record-filters-label-row">
            <label htmlFor="record-filter-timeslots">Select Time Slots</label>
            {filters.timeSlots.length > 0 && (
              <button type="button" className="link-button" onClick={() => set('timeSlots', [])}>Deselect All</button>
            )}
          </div>
          <MultiSelect
            id="record-filter-timeslots"
            options={timeSlotOptions}
            selected={filters.timeSlots}
            onChange={(next) => set('timeSlots', next)}
            placeholder="Search time slots…"
            emptyMessage="No time slots available for the selected filters"
          />
        </div>

        <div className="form-group">
          <label htmlFor="record-filter-status">Status</label>
          <select id="record-filter-status" value={filters.markingStatus} onChange={(e) => set('markingStatus', e.target.value)}>
            {MARKING_STATUS_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
          </select>
        </div>

        <div className="form-group">
          <label htmlFor="record-filter-search">Search</label>
          <input
            id="record-filter-search"
            type="text"
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            placeholder="Search students by name or registration…"
          />
        </div>
      </div>
    </div>
  );
};

export default SessionRecordFilters;
