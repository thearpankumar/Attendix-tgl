const EmptyState = ({ icon: Icon, title, message, action }) => (
  <div className="card empty-state">
    {Icon && (
      <div className="empty-state-icon">
        <Icon size={32} />
      </div>
    )}
    {title && <h4>{title}</h4>}
    {message && <p>{message}</p>}
    {action}
  </div>
);

export default EmptyState;
