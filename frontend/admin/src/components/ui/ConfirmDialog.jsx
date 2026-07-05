import Modal from './Modal';

const ConfirmDialog = ({
  open,
  onClose,
  onSubmit,
  title,
  message,
  children,
  confirmLabel = 'Confirm',
  loading = false,
  danger = true,
}) => (
  <Modal open={open} onClose={onClose} title={title}>
    {message && <p style={{ marginBottom: 'var(--space-4)' }}>{message}</p>}
    <form onSubmit={onSubmit}>
      {children}
      <div className="form-actions">
        <button
          type="submit"
          className={danger ? 'btn btn-delete' : 'btn btn-primary'}
          disabled={loading}
        >
          {loading ? 'Please wait...' : confirmLabel}
        </button>
        <button type="button" className="btn btn-secondary" onClick={onClose} disabled={loading}>
          Cancel
        </button>
      </div>
    </form>
  </Modal>
);

export default ConfirmDialog;
