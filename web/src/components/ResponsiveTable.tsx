import type { ReactNode } from 'react';
import { t } from '../i18n/messages';
import styles from './ResponsiveTable.module.css';

export interface Column<Row> {
  key: string;
  header: string;
  render: (row: Row) => ReactNode;
}

interface ResponsiveTableProps<Row> {
  caption: string;
  columns: Column<Row>[];
  rows: Row[];
  rowKey: (row: Row) => string;
  emptyState?: string;
  testId?: string;
}

/** A real table that stacks into labelled cards on narrow screens (no horizontal page scroll). */
export function ResponsiveTable<Row>({
  caption,
  columns,
  rows,
  rowKey,
  emptyState,
  testId = 'table',
}: ResponsiveTableProps<Row>) {
  if (rows.length === 0) {
    return (
      <p className={styles.empty} data-testid={`${testId}-empty`}>
        {emptyState ?? t('tableEmpty')}
      </p>
    );
  }
  return (
    <table className={styles.table} data-testid={testId}>
      <caption className="visually-hidden">{caption}</caption>
      <thead>
        <tr>
          {columns.map((column) => (
            <th key={column.key} scope="col">
              {column.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={rowKey(row)} data-testid={`${testId}-row`}>
            {columns.map((column) => (
              <td key={column.key} data-label={column.header}>
                {column.render(row)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
