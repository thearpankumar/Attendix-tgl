const PageHeader = ({ title, children }) => (
  <div className="page-header">
    <h2>{title}</h2>
    {children && <div className="page-header-actions">{children}</div>}
  </div>
);

export default PageHeader;
