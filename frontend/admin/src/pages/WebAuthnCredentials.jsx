import { useState, useEffect } from 'react';
import { toast } from 'react-toastify';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:5000';

function WebAuthnCredentials() {
  const [credentials, setCredentials] = useState([]);
  const [stats, setStats] = useState(null);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [suspendedFilter, setSuspendedFilter] = useState('all');
  const [page, setPage] = useState(1);
  const [pagination, setPagination] = useState({ pages: 1, total: 0 });
  const [showResetModal, setShowResetModal] = useState(false);
  const [showSuspendModal, setShowSuspendModal] = useState(false);
  const [selectedStudent, setSelectedStudent] = useState(null);
  const [reason, setReason] = useState('');
  const [actionLoading, setActionLoading] = useState(false);

  const token = localStorage.getItem('token');

  useEffect(() => {
    fetchCredentials();
    fetchStats();
  }, [search, suspendedFilter, page]);

  const fetchCredentials = async () => {
    try {
      setLoading(true);
      const params = new URLSearchParams({
        page: page.toString(),
        limit: '20',
      });
      
      if (search) params.append('search', search);
      if (suspendedFilter !== 'all') params.append('suspended', suspendedFilter);
      
      const res = await fetch(`${API_BASE}/api/admin/webauthn/credentials?${params}`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      
      const data = await res.json();
      
      if (res.ok) {
        setCredentials(data.credentials);
        setPagination(data.pagination);
      } else {
        toast.error(data.message);
      }
    } catch (error) {
      toast.error('Failed to fetch credentials');
    } finally {
      setLoading(false);
    }
  };

  const fetchStats = async () => {
    try {
      const res = await fetch(`${API_BASE}/api/admin/webauthn/stats`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      
      const data = await res.json();
      
      if (res.ok) {
        setStats(data);
      }
    } catch (error) {
      console.error('Failed to fetch stats');
    }
  };

  const handleReset = async () => {
    if (!reason.trim()) {
      toast.error('Please provide a reason');
      return;
    }
    
    setActionLoading(true);
    
    try {
      const res = await fetch(`${API_BASE}/api/admin/webauthn/reset`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({
          rollNumber: selectedStudent.studentId,
          reason: reason.trim(),
        }),
      });
      
      const data = await res.json();
      
      if (res.ok) {
        toast.success(data.message);
        setShowResetModal(false);
        setSelectedStudent(null);
        setReason('');
        fetchCredentials();
        fetchStats();
      } else {
        toast.error(data.message);
      }
    } catch (error) {
      toast.error('Failed to reset credential');
    } finally {
      setActionLoading(false);
    }
  };

  const handleSuspend = async (suspend) => {
    if (!reason.trim()) {
      toast.error('Please provide a reason');
      return;
    }
    
    setActionLoading(true);
    
    try {
      const endpoint = suspend ? 'suspend' : 'unsuspend';
      const res = await fetch(`${API_BASE}/api/admin/webauthn/${endpoint}`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({
          rollNumber: selectedStudent.studentId,
          reason: reason.trim(),
        }),
      });
      
      const data = await res.json();
      
      if (res.ok) {
        toast.success(data.message);
        setShowSuspendModal(false);
        setSelectedStudent(null);
        setReason('');
        fetchCredentials();
        fetchStats();
      } else {
        toast.error(data.message);
      }
    } catch (error) {
      toast.error(`Failed to ${suspend ? 'suspend' : 'unsuspend'} credential`);
    } finally {
      setActionLoading(false);
    }
  };

  const formatDate = (date) => {
    return new Date(date).toLocaleString();
  };

  return (
    <div>
      <h1 style={{ marginBottom: '20px' }}>Biometric Devices</h1>
      
      {/* Stats Cards */}
      {stats && (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '15px', marginBottom: '30px' }}>
          <div className="card" style={{ padding: '20px', textAlign: 'center' }}>
            <h3 style={{ color: '#667eea', margin: 0 }}>{stats.totalEnrolled}</h3>
            <p style={{ color: '#666', margin: '5px 0 0' }}>Total Enrolled</p>
          </div>
          <div className="card" style={{ padding: '20px', textAlign: 'center' }}>
            <h3 style={{ color: '#28a745', margin: 0 }}>{stats.active}</h3>
            <p style={{ color: '#666', margin: '5px 0 0' }}>Active</p>
          </div>
          <div className="card" style={{ padding: '20px', textAlign: 'center' }}>
            <h3 style={{ color: '#dc3545', margin: 0 }}>{stats.suspended}</h3>
            <p style={{ color: '#666', margin: '5px 0 0' }}>Suspended</p>
          </div>
          <div className="card" style={{ padding: '20px', textAlign: 'center' }}>
            <h3 style={{ color: '#17a2b8', margin: 0 }}>{stats.enrollmentTrends?.last7Days || 0}</h3>
            <p style={{ color: '#666', margin: '5px 0 0' }}>Last 7 Days</p>
          </div>
        </div>
      )}
      
      {/* Filters */}
      <div className="card" style={{ marginBottom: '20px', padding: '15px' }}>
        <div style={{ display: 'flex', gap: '15px', flexWrap: 'wrap', alignItems: 'center' }}>
          <input
            type="text"
            placeholder="Search by roll number..."
            value={search}
            onChange={(e) => { setSearch(e.target.value); setPage(1); }}
            style={{ padding: '10px', flex: 1, minWidth: '200px' }}
          />
          <select
            value={suspendedFilter}
            onChange={(e) => { setSuspendedFilter(e.target.value); setPage(1); }}
            style={{ padding: '10px' }}
          >
            <option value="all">All Status</option>
            <option value="false">Active Only</option>
            <option value="true">Suspended Only</option>
          </select>
        </div>
      </div>
      
      {/* Credentials Table */}
      <div className="card">
        {loading ? (
          <p style={{ textAlign: 'center', padding: '20px' }}>Loading...</p>
        ) : credentials.length === 0 ? (
          <p style={{ textAlign: 'center', padding: '20px', color: '#666' }}>No credentials found</p>
        ) : (
          <>
            <table className="table">
              <thead>
                <tr>
                  <th>Roll Number</th>
                  <th>Device</th>
                  <th>Enrolled</th>
                  <th>Last Used</th>
                  <th>Status</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {credentials.map((cred) => (
                  <tr key={cred._id}>
                    <td><strong>{cred.studentId}</strong></td>
                    <td>{cred.deviceLabel || 'Unknown'}</td>
                    <td>{formatDate(cred.enrolledAt)}</td>
                    <td>{cred.lastUsedAt ? formatDate(cred.lastUsedAt) : 'Never'}</td>
                    <td>
                      <span style={{
                        padding: '5px 10px',
                        borderRadius: '4px',
                        fontSize: '12px',
                        background: cred.isSuspended ? '#dc3545' : '#28a745',
                        color: 'white',
                      }}>
                        {cred.isSuspended ? 'Suspended' : 'Active'}
                      </span>
                    </td>
                    <td>
                      <div style={{ display: 'flex', gap: '5px' }}>
                        <button
                          className="btn btn-secondary btn-small"
                          onClick={() => { setSelectedStudent(cred); setShowResetModal(true); }}
                        >
                          Reset
                        </button>
                        {cred.isSuspended ? (
                          <button
                            className="btn btn-small"
                            style={{ background: '#28a745', color: 'white' }}
                            onClick={() => { setSelectedStudent(cred); setShowSuspendModal(true); }}
                          >
                            Unsuspend
                          </button>
                        ) : (
                          <button
                            className="btn btn-small"
                            style={{ background: '#dc3545', color: 'white' }}
                            onClick={() => { setSelectedStudent(cred); setShowSuspendModal(true); }}
                          >
                            Suspend
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            
            {/* Pagination */}
            {pagination.pages > 1 && (
              <div style={{ marginTop: '15px', display: 'flex', justifyContent: 'center', gap: '5px' }}>
                <button
                  className="btn btn-secondary btn-small"
                  disabled={page === 1}
                  onClick={() => setPage(p => p - 1)}
                >
                  Previous
                </button>
                <span style={{ padding: '10px' }}>
                  Page {page} of {pagination.pages}
                </span>
                <button
                  className="btn btn-secondary btn-small"
                  disabled={page === pagination.pages}
                  onClick={() => setPage(p => p + 1)}
                >
                  Next
                </button>
              </div>
            )}
          </>
        )}
      </div>
      
      {/* Reset Modal */}
      {showResetModal && (
        <div className="modal-overlay" onClick={() => setShowResetModal(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>Reset Biometric Credential</h3>
            <p style={{ margin: '15px 0' }}>
              This will delete the biometric credential for <strong>{selectedStudent?.studentId}</strong>.
              The student will need to re-enroll their device.
            </p>
            <div className="form-group">
              <label>Reason (required)</label>
              <textarea
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                placeholder="Enter reason for reset..."
                style={{ width: '100%', padding: '10px', minHeight: '80px' }}
              />
            </div>
            <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
              <button className="btn btn-secondary" onClick={() => setShowResetModal(false)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={handleReset}
                disabled={actionLoading}
              >
                {actionLoading ? 'Resetting...' : 'Reset Credential'}
              </button>
            </div>
          </div>
        </div>
      )}
      
      {/* Suspend/Unsuspend Modal */}
      {showSuspendModal && (
        <div className="modal-overlay" onClick={() => setShowSuspendModal(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>{selectedStudent?.isSuspended ? 'Unsuspend' : 'Suspend'} Credential</h3>
            <p style={{ margin: '15px 0' }}>
              {selectedStudent?.isSuspended
                ? `This will unsuspend the credential for ${selectedStudent?.studentId}. They will be able to use biometric authentication again.`
                : `This will suspend the credential for ${selectedStudent?.studentId}. They will not be able to use biometric authentication.`
              }
            </p>
            <div className="form-group">
              <label>Reason (required)</label>
              <textarea
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                placeholder="Enter reason..."
                style={{ width: '100%', padding: '10px', minHeight: '80px' }}
              />
            </div>
            <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
              <button className="btn btn-secondary" onClick={() => setShowSuspendModal(false)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={() => handleSuspend(!selectedStudent?.isSuspended)}
                disabled={actionLoading}
              >
                {actionLoading ? 'Processing...' : selectedStudent?.isSuspended ? 'Unsuspend' : 'Suspend'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default WebAuthnCredentials;
