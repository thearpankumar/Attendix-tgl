import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useNavigate } from 'react-router';
import { Users, Upload, X, Trash2, FileSpreadsheet, Plus, Eye, Calendar, CalendarClock } from 'lucide-react';
import axios from 'axios';
import { toast } from 'react-toastify';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import Badge from '../components/ui/Badge';

interface Batch {
  _id: string;
  name: string;
  description: string;
  studentCount: number;
  createdAt: string;
  /** "manual" (typed/uploaded roster) | "session" (Excel-upload bulk session creation) */
  type: 'manual' | 'session';
  linkedSessionId?: string;
}

interface Student {
  rollNumber: string;
  name: string;
  collegeName?: string;
  email?: string;
}

interface DetailedBatch extends Batch {
  students: Student[];
}

const Batches = () => {
  const navigate = useNavigate();
  const [batches, setBatches] = useState<Batch[]>([]);
  const [loading, setLoading] = useState(true);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  
  // Drawer State
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);
  const [selectedBatch, setSelectedBatch] = useState<DetailedBatch | null>(null);
  const [drawerLoading, setDrawerLoading] = useState(false);

  // Form State
  const [formData, setFormData] = useState({ name: '', description: '' });
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    fetchBatches();
  }, []);

  const fetchBatches = async () => {
    try {
      const { data } = await axios.get('/api/admin/batches');
      setBatches(data);
    } catch (_err) {
      toast.error('Failed to load batches');
    } finally {
      setLoading(false);
    }
  };

  const openBatchDetails = async (batch: Batch) => {
    // Session-batches (Excel-upload bulk session creation) aren't
    // independently browsable rosters — they're always 1:1 with the
    // session that generated them, so "view" deep-links there instead of
    // opening the plain student-roster drawer used for manual batches.
    if (batch.type === 'session') {
      if (batch.linkedSessionId) navigate(`/sessions/${batch.linkedSessionId}`);
      return;
    }
    setIsDrawerOpen(true);
    setDrawerLoading(true);
    try {
      const { data } = await axios.get(`/api/admin/batches/${batch._id}`);
      setSelectedBatch(data);
    } catch (_err) {
      toast.error('Failed to fetch batch details');
      setIsDrawerOpen(false);
    } finally {
      setDrawerLoading(false);
    }
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  };

  const handleDragLeave = () => {
    setIsDragging(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const file = e.dataTransfer.files[0];
    validateAndSetFile(file);
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) validateAndSetFile(file);
  };

  const validateAndSetFile = (file: File) => {
    const validTypes = [
      'text/csv',
      'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      'application/vnd.ms-excel',
      'application/vnd.oasis.opendocument.spreadsheet'
    ];
    const nameLower = file.name.toLowerCase();
    if (!validTypes.includes(file.type) && !nameLower.endsWith('.csv') && !nameLower.endsWith('.xlsx') && !nameLower.endsWith('.xls') && !nameLower.endsWith('.ods')) {
      toast.error('Please upload a valid .csv, .xlsx, .xls, or .ods file');
      return;
    }
    if (file.size > 10 * 1024 * 1024) {
      toast.error('File size must be less than 10MB');
      return;
    }
    setSelectedFile(file);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedFile) {
      toast.error('Please select a file to import');
      return;
    }

    setSubmitting(true);
    const data = new FormData();
    data.append('name', formData.name);
    data.append('description', formData.description);
    data.append('file', selectedFile);

    try {
      await axios.post('/api/admin/batches', data, {
        headers: { 'Content-Type': 'multipart/form-data' }
      });
      toast.success('Batch created successfully!');
      setIsModalOpen(false);
      setFormData({ name: '', description: '' });
      setSelectedFile(null);
      fetchBatches();
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to create batch');
    } finally {
      setSubmitting(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!window.confirm('Are you sure you want to delete this batch? This will not delete past attendance records, but will unlink it from active sessions.')) return;
    
    setDeletingId(id);
    try {
      await axios.delete(`/api/admin/batches/${id}`);
      toast.success('Batch deleted');
      setBatches(batches.filter(b => b._id !== id));
    } catch (_err) {
      toast.error('Failed to delete batch');
    } finally {
      setDeletingId(null);
    }
  };

  const columns: Column<Batch>[] = [
    {
      key: 'name',
      label: 'Batch Name',
      render: (b: Batch) => (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span>{b.name}</span>
          {b.type === 'session' && (
            <Badge tone="neutral">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                <CalendarClock size={11} /> Session Batch
              </span>
            </Badge>
          )}
        </div>
      ),
    },
    { key: 'description', label: 'Description' },
    {
      key: 'students',
      label: 'Students',
      render: (b: Batch) => (
        <div
          className="status-badge status-success"
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '6px',
            padding: '4px 12px'
          }}
        >
          <Users size={14} />
          <span>{b.studentCount}</span>
        </div>
      )
    },
    {
      key: 'created',
      label: 'Created',
      render: (b: Batch) => new Date(b.createdAt).toLocaleDateString()
    },
    {
      key: 'actions',
      label: 'Actions',
      render: (b: Batch) => (
        <div style={{ display: 'flex', gap: '8px', justifyContent: 'center' }}>
          <button
            className="btn btn-secondary btn-small"
            onClick={() => openBatchDetails(b)}
            title={b.type === 'session' ? 'Open session' : 'View Details'}
          >
            <Eye size={14} />
          </button>
          {b.type === 'manual' && (
            <button
              className="btn btn-danger btn-small"
              onClick={() => handleDelete(b._id)}
              disabled={deletingId === b._id}
              title="Delete Batch"
            >
              <Trash2 size={14} />
            </button>
          )}
        </div>
      )
    }
  ];

  return (
    <div className="container">
      <div className="page-header" style={{ marginBottom: '2rem', gap: '16px' }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
          <h1 className="page-title" style={{ fontSize: '2.2rem', fontWeight: 800, margin: 0, letterSpacing: '-0.5px', color: 'var(--text-color)' }}>
            Batches & Rosters
          </h1>
          <p className="page-subtitle" style={{ fontSize: '1.05rem', color: 'var(--text-muted)', margin: 0, fontWeight: 500 }}>
            Import student rosters to seamlessly track absentees across your active sessions.
          </p>
        </div>
        <button className="btn btn-primary" onClick={() => setIsModalOpen(true)} style={{ padding: '10px 20px', fontWeight: 600, fontSize: '1rem', boxShadow: '0 4px 12px rgba(var(--primary-rgb), 0.2)' }}>
          <Plus size={20} style={{ marginRight: '6px' }} />
          Create Batch
        </button>
      </div>

      <div className="card card-table">
        {loading ? (
          <div className="loading">Loading batches...</div>
        ) : batches.length === 0 ? (
          <div className="empty-state">
            <Users size={48} className="empty-icon" />
            <h3>No Batches Found</h3>
            <p>You haven't created any student batches yet.</p>
            <button className="btn btn-primary" onClick={() => setIsModalOpen(true)}>
              Import Your First Roster
            </button>
          </div>
        ) : (
          <DataTable rows={batches} columns={columns} rowKey={(b) => b._id} />
        )}
      </div>

      <AnimatePresence>
        {isModalOpen && (
          <div className="modal-overlay">
            <motion.div
              className="modal-content"
              initial={{ opacity: 0, scale: 0.95, y: 20 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95, y: 20 }}
            >
              <div className="modal-header">
                <h2>Import Student Batch</h2>
                <button className="close-btn" onClick={() => setIsModalOpen(false)}>
                  <X size={20} />
                </button>
              </div>

              <form onSubmit={handleSubmit} className="modal-body">
                <div className="form-group">
                  <label htmlFor="batchName">Batch Name</label>
                  <input
                    type="text"
                    id="batchName"
                    value={formData.name}
                    onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                    placeholder="e.g. CS 3rd Year (2024)"
                    required
                  />
                </div>

                <div className="form-group">
                  <label htmlFor="batchDesc">Description (Optional)</label>
                  <input
                    type="text"
                    id="batchDesc"
                    value={formData.description}
                    onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                    placeholder="e.g. Morning Shift Students"
                  />
                </div>

                <div className="form-group">
                  <label>Roster File (CSV / Excel)</label>
                  <div
                    className={`file-drop-zone ${isDragging ? 'dragging' : ''} ${selectedFile ? 'has-file' : ''}`}
                    onDragOver={handleDragOver}
                    onDragLeave={handleDragLeave}
                    onDrop={handleDrop}
                    onClick={() => fileInputRef.current?.click()}
                  >
                    <input
                      type="file"
                      ref={fileInputRef}
                      onChange={handleFileChange}
                      accept=".csv, application/vnd.openxmlformats-officedocument.spreadsheetml.sheet, application/vnd.ms-excel"
                      style={{ display: 'none' }}
                    />
                    
                    {selectedFile ? (
                      <div className="file-selected-info">
                        <FileSpreadsheet size={32} className="text-primary" />
                        <span className="file-name">{selectedFile.name}</span>
                        <span className="file-size">{(selectedFile.size / 1024).toFixed(1)} KB</span>
                        <p className="click-to-change">Click to change file</p>
                      </div>
                    ) : (
                      <>
                        <Upload size={32} className="text-muted" />
                        <p>Drag & drop your file here, or click to browse</p>
                        <span className="text-muted" style={{ fontSize: '0.8rem' }}>
                          Supports: Roll / Reg No, Student Name (any column order)
                        </span>
                      </>
                    )}
                  </div>
                </div>

                <div className="form-actions">
                  <button type="submit" className="btn btn-primary" disabled={submitting || !selectedFile}>
                    {submitting ? 'Importing...' : 'Create Batch'}
                  </button>
                  <button type="button" className="btn btn-secondary" onClick={() => setIsModalOpen(false)}>
                    Cancel
                  </button>
                </div>
              </form>
            </motion.div>
          </div>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {isDrawerOpen && (
          <>
            <motion.div
              className="drawer-backdrop"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              onClick={() => setIsDrawerOpen(false)}
              style={{
                position: 'fixed',
                top: 0, left: 0, right: 0, bottom: 0,
                backgroundColor: 'rgba(0,0,0,0.5)',
                backdropFilter: 'blur(4px)',
                zIndex: 999
              }}
            />
            <motion.div
              className="drawer-panel neumorphic-modal"
              initial={{ x: '100%' }}
              animate={{ x: 0 }}
              exit={{ x: '100%' }}
              transition={{ type: 'spring', damping: 25, stiffness: 200 }}
              style={{
                position: 'fixed',
                top: 0,
                right: 0,
                bottom: 0,
                width: '100%',
                maxWidth: '480px',
                zIndex: 1000,
                margin: 0,
                borderRadius: '24px 0 0 24px',
                overflow: 'hidden',
                display: 'flex',
                flexDirection: 'column',
                height: '100vh',
                boxShadow: '-10px 0 30px rgba(0,0,0,0.15)'
              }}
            >
              <style>{`
                @import url('https://fonts.googleapis.com/css2?family=Manrope:wght@400;500;600;700&family=Poppins:wght@500;600;700;800&display=swap');
                
                /* Light Mode Styles */
                .neumorphic-modal {
                  font-family: 'Manrope', sans-serif;
                  color: #334155;
                  background-color: #eef2f6;
                  border-left: 1px solid rgba(163, 177, 198, 0.3);
                }
                .neumorphic-title {
                  font-family: 'Poppins', sans-serif;
                  color: #1e293b;
                }
                .neumorphic-card {
                  background: #eef2f6;
                  border-radius: 16px;
                  box-shadow: 6px 6px 12px rgba(163, 177, 198, 0.4), -6px -6px 12px rgba(255, 255, 255, 0.9);
                  padding: 16px;
                  display: flex;
                  align-items: center;
                  gap: 12px;
                }
                .neumorphic-table-container {
                  background: #eef2f6;
                  border-radius: 16px;
                  box-shadow: inset 4px 4px 8px rgba(163, 177, 198, 0.4), inset -4px -4px 8px rgba(255, 255, 255, 0.9);
                  padding: 12px;
                  overflow-y: auto;
                }
                .neumorphic-table th {
                  font-family: 'Poppins', sans-serif;
                  font-size: 0.75rem;
                  text-transform: uppercase;
                  color: #64748b;
                  padding: 12px 16px;
                  border-bottom: 2px solid rgba(163,177,198,0.2);
                  text-align: left;
                }
                .neumorphic-table td {
                  padding: 12px 16px;
                  border-bottom: 1px solid rgba(163,177,198,0.2);
                  font-size: 0.85rem;
                  font-weight: 500;
                  color: #334155;
                }
                .neumorphic-table tr:last-child td { border-bottom: none; }

                /* Dark Mode overrides */
                .dark .neumorphic-modal {
                  color: #cbd5e1;
                  background-color: #16192e;
                  border-left: 1px solid rgba(255, 255, 255, 0.05);
                }
                .dark .neumorphic-title {
                  color: #f8fafc;
                }
                .dark .neumorphic-card {
                  background: #16192e;
                  box-shadow: 6px 6px 12px rgba(0, 0, 0, 0.4), -6px -6px 12px rgba(255, 255, 255, 0.05);
                }
                .dark .neumorphic-table-container {
                  background: #16192e;
                  box-shadow: inset 4px 4px 8px rgba(0, 0, 0, 0.4), inset -4px -4px 8px rgba(255, 255, 255, 0.05);
                }
                .dark .neumorphic-table th {
                  color: #94a3b8;
                  border-bottom: 2px solid rgba(255,255,255,0.05);
                }
                .dark .neumorphic-table td {
                  color: #e2e8f0;
                  border-bottom: 1px solid rgba(255,255,255,0.05);
                }
              `}</style>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', padding: '28px 28px 16px 28px' }}>
                <div>
                  <h2 className="neumorphic-title" style={{ margin: '0 0 4px 0', fontSize: '1.75rem', fontWeight: 700 }}>
                    {selectedBatch?.name || 'Loading...'}
                  </h2>
                  <p style={{ margin: 0, color: '#64748b', fontSize: '1rem', fontWeight: 500 }} className="dark:text-slate-400">
                    {selectedBatch?.description || 'No description provided.'}
                  </p>
                </div>
                <button onClick={() => setIsDrawerOpen(false)} style={{ background: 'none', border: 'none', color: '#64748b', cursor: 'pointer', padding: '4px' }}>
                  <X size={24} />
                </button>
              </div>
              
              <div style={{ padding: '0 28px 28px 28px', flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column', gap: '24px' }}>
                {drawerLoading ? (
                  <div style={{ padding: '40px 0', textAlign: 'center' }}>Loading...</div>
                ) : selectedBatch?.students ? (
                  <>
                    <div style={{ display: 'flex', gap: '16px', flexShrink: 0 }}>
                      <div className="neumorphic-card" style={{ flex: 1 }}>
                        <div style={{ color: '#94a3b8' }}><Users size={24} strokeWidth={1.5} /></div>
                        <div>
                          <div style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 600, lineHeight: 1.2 }} className="dark:text-slate-400">Total<br/>Students</div>
                          <div className="neumorphic-title" style={{ fontSize: '1.25rem', fontWeight: 700, lineHeight: 1.2, marginTop: '4px' }}>{selectedBatch.students.length}</div>
                        </div>
                      </div>
                      <div className="neumorphic-card" style={{ flex: 1 }}>
                        <div style={{ color: '#94a3b8' }}><Calendar size={24} strokeWidth={1.5} /></div>
                        <div>
                          <div style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 600, lineHeight: 1.2 }} className="dark:text-slate-400">Created On</div>
                          <div className="neumorphic-title" style={{ fontSize: '1rem', fontWeight: 700, lineHeight: 1.2, marginTop: '4px' }}>
                            {new Date(selectedBatch.createdAt).toLocaleDateString()}
                          </div>
                        </div>
                      </div>
                    </div>

                    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', minHeight: 0 }}>
                      <h3 className="neumorphic-title" style={{ fontSize: '1rem', fontWeight: 700, marginBottom: '12px', flexShrink: 0 }}>Student Roster</h3>
                      <div className="neumorphic-table-container" style={{ flex: 1, overflowY: 'auto' }}>
                        <table className="neumorphic-table" style={{ width: '100%', borderCollapse: 'collapse' }}>
                          <thead>
                            <tr>
                              <th>Roll Number</th>
                              <th>Name</th>
                            </tr>
                          </thead>
                          <tbody>
                            {selectedBatch.students.map((s: Student) => (
                              <tr key={s.rollNumber}>
                                <td style={{ color: '#94a3b8' }}>{s.rollNumber}</td>
                                <td>{s.name}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    </div>
                  </>
                ) : (
                  <div style={{ textAlign: 'center', color: '#64748b' }} className="dark:text-slate-400">Failed to load batch data.</div>
                )}
              </div>
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  );
};

export default Batches;
