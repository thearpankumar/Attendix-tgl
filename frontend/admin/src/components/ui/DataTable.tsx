import { Fragment } from 'react';
import type { ReactNode } from 'react';

export interface Column<T> {
  key: string;
  label: ReactNode;
  align?: 'left' | 'right' | 'center';
  width?: string;
  render?: (row: T) => ReactNode;
}

interface DataTableProps<T> {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  /** Key of the currently expanded row, if any (opt-in — omit for a plain table). */
  expandedRowKey?: string | null;
  /** Rendered as a full-width row directly under the row whose key matches `expandedRowKey`. */
  renderExpandedRow?: (row: T) => ReactNode;
}

function DataTable<T>({ columns, rows, rowKey, expandedRowKey, renderExpandedRow }: DataTableProps<T>) {
  if (!rows || rows.length === 0) return null;

  return (
    <div className="table-scroll">
      <table className="table">
        <colgroup>
          {columns.map((col) => (
            <col key={col.key} style={{ width: col.width ?? `${100 / columns.length}%` }} />
          ))}
        </colgroup>
        <thead>
          <tr>
            {columns.map((col) => (
              <th key={col.key} style={{ textAlign: col.align ?? 'center' }}>{col.label}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            const key = rowKey(row);
            const isExpanded = renderExpandedRow && key === expandedRowKey;
            return (
              <Fragment key={key}>
                <tr>
                  {columns.map((col) => (
                    <td key={col.key} data-label={typeof col.label === 'string' ? col.label : ''} style={{ textAlign: col.align ?? 'center' }}>
                      {col.render ? col.render(row) : String((row as Record<string, unknown>)[col.key] ?? '')}
                    </td>
                  ))}
                </tr>
                {isExpanded && (
                  <tr>
                    <td colSpan={columns.length} style={{ padding: 0 }}>
                      {renderExpandedRow!(row)}
                    </td>
                  </tr>
                )}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export default DataTable;
