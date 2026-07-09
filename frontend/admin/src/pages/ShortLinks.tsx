import { useEffect, useState, useCallback } from 'react';
import type { FormEvent } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { Copy, Link2 } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import Modal from '../components/ui/Modal';
import Badge from '../components/ui/Badge';
import Button from '../components/ui/Button';
import ConfirmModal from '../components/ui/ConfirmModal';
import { SkeletonRows } from '../components/ui/Skeleton';

interface Session {
  _id: string;
  isActive: boolean;
  expiresAt: string;
  description?: string;
  locationId?: { name: string };
}

interface ShortLink {
  _id: string;
  shortCode: string;
  isActive: boolean;
  clickCount?: number;
  createdAt: string;
  sessionId?: { _id: string; isActive: boolean; expiresAt: string; description?: string };
}

const ShortLinks = () => {
  const [shortLinks, setShortLinks] = useState<ShortLink[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [formData, setFormData] = useState({ shortCode: '', sessionId: '' });
  const [autoGenerate, setAutoGenerate] = useState(true);
  const [confirmAction, setConfirmAction] = useState<{ type: 'detach' | 'delete'; shortCode: string } | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const [linksRes, sessionsRes] = await Promise.all([
        axios.get<{ shortLinks: ShortLink[] }>('/api/admin/shortlinks'),
        axios.get<Session[]>('/api/admin/sessions'),
      ]);
      setShortLinks(linksRes.data.shortLinks);
      setSessions(sessionsRes.data);
    } catch { toast.error('Failed to fetch data'); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { fetchData(); }, [fetchData]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    try {
      const payload = {
        shortCode: autoGenerate ? '' : formData.shortCode.trim().toLowerCase(),
        sessionId: formData.sessionId || null,
      };
      const res = await axios.post<{ shortCode: string }>('/api/admin/shortlinks', payload);
      toast.success(`Short link created: ${res.data.shortCode}`);
      setShowModal(false);
      setFormData({ shortCode: '', sessionId: '' });
      fetchData();
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to create short link');
    }
  };

  const handleAttach = async (shortCode: string, sessionId: string) => {
    try { await axios.post(`/api/admin/shortlinks/${shortCode}/attach`, { sessionId }); toast.success('Short link attached'); fetchData(); }
    catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to attach');
    }
  };

  const handleDetach = async (shortCode: string) => {
    try { await axios.post(`/api/admin/shortlinks/${shortCode}/detach`); toast.success('Short link detached'); fetchData(); }
    catch { toast.error('Failed to detach'); }
    setConfirmAction(null);
  };

  const handleDelete = async (shortCode: string) => {
    try { await axios.delete(`/api/admin/shortlinks/${shortCode}`); toast.success('Short link deleted'); fetchData(); }
    catch { toast.error('Failed to delete'); }
    setConfirmAction(null);
  };

  const getFullUrl = (shortCode: string) => {
    // Short links resolve through the Caddy front door (port 80/443), never the
    // admin's own dev port (:3000), which has no /s/ route. Emit a port-less URL.
    const { protocol, hostname } = window.location;
    return `${protocol}//${hostname}/s/${shortCode}`;
  };

  const activeSessions = sessions.filter((s) => s.isActive && new Date(s.expiresAt) > new Date());

  const columns: Column<ShortLink>[] = [
    { key: 'shortCode', label: 'Short Code', width: '10%', render: (l) => <code>{l.shortCode}</code> },
    { key: 'url', label: 'Full URL', width: '22%', render: (l) => {
      const url = getFullUrl(l.shortCode);
      return (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px', minWidth: 0 }}>
          <a href={url} target="_blank" rel="noopener noreferrer" style={{ fontSize: '13px', flex: 1, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{url}</a>
          <Button variant="secondary" size="sm" onClick={() => { navigator.clipboard.writeText(url); toast.success('Link copied!'); }} aria-label="Copy link"><Copy size={12} /></Button>
        </div>
      );
    }},
    { key: 'session', label: 'Attached Session', width: '20%', render: (l) => l.sessionId ? (
      <div style={{ minWidth: 0 }}>
        <div style={{ fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{l.sessionId.description || 'Session'}</div>
        <div style={{ fontSize: '12px', color: 'var(--color-muted)' }}>Expires: {new Date(l.sessionId.expiresAt).toLocaleString()}</div>
      </div>
    ) : <span style={{ color: 'var(--color-faint)' }}>Not attached</span> },
    { key: 'status', label: 'Status', width: '11%', render: (l) => {
      const isActive = l.isActive && l.sessionId?.isActive;
      return <Badge tone={isActive ? 'success' : 'warning'}>{isActive ? 'Active' : 'Inactive'}</Badge>;
    }},
    { key: 'clicks',  label: 'Clicks',  width: '8%',  align: 'center', render: (l) => l.clickCount || 0 },
    { key: 'created', label: 'Created', width: '11%', render: (l) => new Date(l.createdAt).toLocaleDateString() },
    { key: 'actions', label: 'Actions', width: '18%', render: (l) => (
      <div className="actions-cell">
        {l.sessionId ? (
          <Button variant="secondary" size="sm" onClick={() => setConfirmAction({ type: 'detach', shortCode: l.shortCode })}>Detach</Button>
        ) : (
          <select
            className="btn btn-small btn-secondary"
            onChange={(e) => { if (e.target.value) { handleAttach(l.shortCode, e.target.value); e.target.value = ''; } }}
            defaultValue=""
          >
            <option value="" disabled>Attach to...</option>
            {activeSessions.map((s) => <option key={s._id} value={s._id}>{s.description || s.locationId?.name || 'Session'}</option>)}
          </select>
        )}
        <Button variant="delete" size="sm" onClick={() => setConfirmAction({ type: 'delete', shortCode: l.shortCode })}>Delete</Button>
      </div>
    )},
  ];

  return (
    <div className="container">
      <PageHeader title="Short Links Management">
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>Create Short Link</button>
      </PageHeader>

      <div className="info-card">
        <h4>How it works</h4>
        <ul>
          <li><strong>Create</strong> a short link (e.g., <code>myclass123</code>)</li>
          <li><strong>Attach</strong> it to an active attendance session</li>
          <li>Share the link: <code>yourdomain.com/s/myclass123</code></li>
          <li>Students see a <strong>time-rotating QR code</strong> that changes every 5 seconds</li>
          <li>Students must scan the current code to mark attendance</li>
        </ul>
      </div>

      {loading ? <SkeletonRows /> : shortLinks.length === 0 ? (
        <EmptyState icon={Link2} title="No short links yet" message="Create your first short link to start sharing attendance sessions." />
      ) : (
        <div className="card card-table">
          <DataTable columns={columns} rows={shortLinks} rowKey={(l) => l._id} />
        </div>
      )}

      <Modal open={showModal} onClose={() => setShowModal(false)} title="Create Short Link">
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label htmlFor="autoGenerate" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <input id="autoGenerate" type="checkbox" style={{ width: 'auto', minHeight: 'unset' }} checked={autoGenerate} onChange={(e) => setAutoGenerate(e.target.checked)} />
              Auto-generate short code
            </label>
          </div>
          {!autoGenerate && (
            <div className="form-group">
              <label htmlFor="shortCode">Custom Short Code</label>
              <input id="shortCode" type="text" value={formData.shortCode} onChange={(e) => setFormData({ ...formData, shortCode: e.target.value })} placeholder="e.g., cs101-monday" pattern="[a-z0-9-]{3,20}" required />
              <small className="form-hint">3-20 characters: lowercase letters, numbers, hyphens only</small>
            </div>
          )}
          <div className="form-group">
            <label htmlFor="sessionId">Attach to Session (optional)</label>
            <select id="sessionId" value={formData.sessionId} onChange={(e) => setFormData({ ...formData, sessionId: e.target.value })}>
              <option value="">Create without attaching</option>
              {activeSessions.map((s) => <option key={s._id} value={s._id}>{s.description || 'Session'} - {s.locationId?.name || 'Unknown'}</option>)}
            </select>
          </div>
          <div className="form-actions">
            <button type="submit" className="btn btn-success">Create Short Link</button>
            <button type="button" className="btn btn-secondary" onClick={() => setShowModal(false)}>Cancel</button>
          </div>
        </form>
      </Modal>

      <ConfirmModal
        isOpen={confirmAction !== null}
        title={confirmAction?.type === 'detach' ? 'Detach Short Link' : 'Delete Short Link'}
        message={confirmAction?.type === 'detach' ? 'Are you sure you want to detach this short link from its session?' : 'Are you sure you want to permanently delete this short link?'}
        confirmText={confirmAction?.type === 'detach' ? 'Detach' : 'Delete'}
        onConfirm={() => {
          if (confirmAction?.type === 'detach') handleDetach(confirmAction.shortCode);
          if (confirmAction?.type === 'delete') handleDelete(confirmAction.shortCode);
        }}
        onCancel={() => setConfirmAction(null)}
      />
    </div>
  );
};

export default ShortLinks;
