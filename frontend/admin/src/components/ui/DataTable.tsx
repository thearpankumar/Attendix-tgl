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
}

function DataTable<T>({ columns, rows, rowKey }: DataTableProps<T>) {
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
          {rows.map((row) => (
            <tr key={rowKey(row)}>
              {columns.map((col) => (
                <td key={col.key} data-label={typeof col.label === 'string' ? col.label : ''} style={{ textAlign: col.align ?? 'center' }}>
                  {col.render ? col.render(row) : String((row as Record<string, unknown>)[col.key] ?? '')}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default DataTable;
