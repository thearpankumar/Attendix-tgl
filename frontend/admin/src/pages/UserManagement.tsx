import { useEffect, useState, useCallback } from 'react';
import type { FormEvent } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { UserCog, Pencil, Trash2, ShieldCheck, Ban, Power } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import Modal from '../components/ui/Modal';
import ConfirmDialog from '../components/ui/ConfirmDialog';
import EmptyState from '../components/ui/EmptyState';
import Badge from '../components/ui/Badge';
import Button from '../components/ui/Button';
import { SkeletonRows } from '../components/ui/Skeleton';
import { useAuth } from '../context/AuthContext';

interface AdminUser {
  _id: string;
  username: string;
  email: string;
  fullName?: string;
  collegeName?: string;
  role: 'admin' | 'super_admin';
  isActive: boolean;
  createdAt: string;
}

const emptyForm = {
  username: '', email: '', fullName: '', collegeName: '', password: '', role: 'admin' as 'admin' | 'super_admin',
};

const UserManagement = () => {
  const { admin: currentAdmin } = useAuth();
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [editing, setEditing] = useState<AdminUser | null>(null);
  const [form, setForm] = useState(emptyForm);
  const [saving, setSaving] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<AdminUser | null>(null);
  const [deleting, setDeleting] = useState(false);

  const fetchUsers = useCallback(async () => {
    try {
      const res = await axios.get<AdminUser[]>('/api/admin/users');
      setUsers(res.data);
    } catch {
      toast.error('Failed to fetch admin/mentor accounts');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetchUsers(); }, [fetchUsers]);

  const openCreate = () => { setEditing(null); setForm(emptyForm); setShowModal(true); };
  const openEdit = (u: AdminUser) => {
    setEditing(u);
    setForm({ username: u.username, email: u.email, fullName: u.fullName || '', collegeName: u.collegeName || '', password: '', role: u.role });
    setShowModal(true);
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      if (editing) {
        const payload: Record<string, unknown> = {
          fullName: form.fullName || null,
          email: form.email,
          collegeName: form.collegeName || null,
        };
        // A super-admin can't touch their own role/active status — omit
        // those fields entirely for a self-edit rather than let the backend
        // reject the whole request.
        if (editing._id !== currentAdmin?._id) payload.role = form.role;
        if (form.password) payload.newPassword = form.password;
        await axios.patch(`/api/admin/users/${editing._id}`, payload);
        toast.success('Account updated');
      } else {
        await axios.post('/api/admin/users', {
          username: form.username,
          email: form.email,
          fullName: form.fullName || null,
          collegeName: form.collegeName || null,
          password: form.password,
          role: form.role,
        });
        toast.success('Account created');
      }
      setShowModal(false);
      fetchUsers();
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to save account');
    } finally {
      setSaving(false);
    }
  };

  const toggleActive = async (u: AdminUser) => {
    if (u._id === currentAdmin?._id) { toast.error("You can't deactivate your own account"); return; }
    try {
      await axios.patch(`/api/admin/users/${u._id}`, { isActive: !u.isActive });
      toast.success(u.isActive ? 'Account deactivated' : 'Account reactivated');
      fetchUsers();
    } catch {
      toast.error('Failed to update account status');
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      await axios.delete(`/api/admin/users/${deleteTarget._id}`);
      toast.success('Account deleted');
      setDeleteTarget(null);
      fetchUsers();
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to delete account');
    } finally {
      setDeleting(false);
    }
  };

  const columns: Column<AdminUser>[] = [
    { key: 'name', label: 'Name', width: '20%', align: 'left', render: (u) => u.fullName || <span style={{ color: 'var(--color-muted)' }}>—</span> },
    { key: 'username', label: 'Username', width: '14%' , render: (u) => u.username },
    { key: 'email', label: 'Email', width: '20%', render: (u) => u.email },
    { key: 'college', label: 'College', width: '16%', render: (u) => u.collegeName || <span style={{ color: 'var(--color-muted)' }}>—</span> },
    { key: 'role', label: 'Role', width: '12%', render: (u) => (
      <Badge tone={u.role === 'super_admin' ? 'warning' : 'neutral'}>{u.role === 'super_admin' ? 'Super Admin' : 'Mentor'}</Badge>
    ) },
    { key: 'status', label: 'Status', width: '10%', render: (u) => (
      <Badge tone={u.isActive ? 'success' : 'danger'}>{u.isActive ? 'Active' : 'Disabled'}</Badge>
    ) },
    { key: 'actions', label: 'Actions', width: '20%', render: (u) => (
      <div className="actions-cell">
        <Button size="sm" onClick={() => openEdit(u)}><Pencil size={13} /></Button>
        <Button
          size="sm"
          variant={u.isActive ? 'danger' : 'success'}
          onClick={() => toggleActive(u)}
          disabled={u._id === currentAdmin?._id}
          aria-label={u.isActive ? 'Disable account' : 'Enable account'}
          title={u._id === currentAdmin?._id ? "You can't deactivate your own account" : (u.isActive ? 'Disable account' : 'Enable account')}
        >
          {u.isActive ? <Ban size={13} /> : <Power size={13} />}
        </Button>
        <Button
          size="sm"
          variant="delete"
          onClick={() => setDeleteTarget(u)}
          disabled={u._id === currentAdmin?._id}
          title={u._id === currentAdmin?._id ? "You can't delete your own account" : undefined}
        >
          <Trash2 size={13} />
        </Button>
      </div>
    ) },
  ];

  return (
    <div className="container">
      <PageHeader title="User Management">
        <button className="btn btn-primary" onClick={openCreate}>Create Account</button>
      </PageHeader>

      {loading ? <SkeletonRows /> : users.length === 0 ? (
        <EmptyState icon={UserCog} title="No admin/mentor accounts yet" message="Create the first mentor account so they can log into the attendance-marking app." />
      ) : (
        <div className="card card-table">
          <DataTable columns={columns} rows={users} rowKey={(u) => u._id} />
        </div>
      )}

      <Modal open={showModal} onClose={() => setShowModal(false)} title={editing ? 'Edit Account' : 'Create Account'}>
        <form onSubmit={handleSubmit}>
          {!editing && (
            <div className="form-group">
              <label htmlFor="user-username">Username</label>
              <input id="user-username" type="text" value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })} minLength={3} maxLength={30} required />
            </div>
          )}
          <div className="form-group">
            <label htmlFor="user-fullname">Full Name</label>
            <input id="user-fullname" type="text" value={form.fullName} onChange={(e) => setForm({ ...form, fullName: e.target.value })} placeholder="e.g., Priya Sharma" />
          </div>
          <div className="form-group">
            <label htmlFor="user-email">College Email</label>
            <input id="user-email" type="email" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} required />
          </div>
          <div className="form-group">
            <label htmlFor="user-college">College Name</label>
            <input id="user-college" type="text" value={form.collegeName} onChange={(e) => setForm({ ...form, collegeName: e.target.value })} placeholder="e.g., XYZ Engineering College" />
          </div>
          <div className="form-group">
            <label htmlFor="user-password">{editing ? 'New Password (leave blank to keep current)' : 'Password'}</label>
            <input id="user-password" type="password" value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })} minLength={6} required={!editing} />
          </div>
          {(!editing || editing._id !== currentAdmin?._id) && (
            <div className="form-group">
              <label htmlFor="user-role">Role</label>
              <select id="user-role" value={form.role} onChange={(e) => setForm({ ...form, role: e.target.value as 'admin' | 'super_admin' })}>
                <option value="admin">Admin / Mentor — marks attendance</option>
                <option value="super_admin">Super Admin — full panel access</option>
              </select>
            </div>
          )}
          {editing && editing._id === currentAdmin?._id && (
            <p style={{ fontSize: '0.8rem', color: 'var(--color-muted)', display: 'flex', alignItems: 'center', gap: 6 }}>
              <ShieldCheck size={14} /> You can't change your own role or active status here.
            </p>
          )}
          <div className="form-actions">
            <button type="submit" className="btn btn-success" disabled={saving}>{saving ? 'Saving…' : editing ? 'Save Changes' : 'Create Account'}</button>
            <button type="button" className="btn btn-secondary" onClick={() => setShowModal(false)}>Cancel</button>
          </div>
        </form>
      </Modal>

      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onSubmit={(e) => { e.preventDefault(); handleDelete(); }}
        title="Delete Account"
        confirmLabel="Delete"
        loading={deleting}
        message={
          <>
            Permanently delete <strong>{deleteTarget?.fullName || deleteTarget?.username}</strong>&apos;s account?
            {' '}If they still own locations, batches, or sessions, this will be rejected until those are reassigned.
          </>
        }
      />
    </div>
  );
};

export default UserManagement;
