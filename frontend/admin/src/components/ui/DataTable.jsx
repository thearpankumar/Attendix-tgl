import { useState } from 'react';

/**
 * columns: [{ key, label, render(row), priority }]
 * priority 3 columns collapse behind a "Show details" toggle on mobile (<640px).
 */
const DataTable = ({ columns, rows, rowKey }) => {
  const [expanded, setExpanded] = useState(() => new Set());
  const hasSecondary = columns.some((c) => c.priority === 3);

  if (!rows || rows.length === 0) return null;

  const toggleRow = (key) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });
  };

  return (
    <div className="table-wrap">
      <table className="table data-table">
        <thead>
          <tr>
            {columns.map((col) => (
              <th key={col.key}>{col.label}</th>
            ))}
            {hasSecondary && <th className="toggle-col" />}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            const key = rowKey(row);
            const isExpanded = expanded.has(key);
            return (
              <tr key={key} className={isExpanded ? 'row-expanded' : ''}>
                {columns.map((col) => (
                  <td
                    key={col.key}
                    data-label={col.label}
                    className={col.priority === 3 ? 'cell-secondary' : ''}
                  >
                    {col.render ? col.render(row) : row[col.key]}
                  </td>
                ))}
                {hasSecondary && (
                  <td className="toggle-col no-label">
                    <button
                      type="button"
                      className="row-details-toggle"
                      onClick={() => toggleRow(key)}
                    >
                      {isExpanded ? 'Hide details' : 'Show details'}
                    </button>
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};

export default DataTable;
