import { cx } from '../styles/cx';
import { createContext, useCallback, useContext, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { t } from '../i18n/messages';
import styles from './Toast.module.css';

export type ToastKind = 'success' | 'error' | 'info';

interface ToastItem {
  id: number;
  kind: ToastKind;
  text: string;
}

interface ToastApi {
  show: (kind: ToastKind, text: string) => void;
}

const ToastContext = createContext<ToastApi | null>(null);
const SUCCESS_VISIBLE_MS = 6000;

/**
 * Outcome messages. Successes and information are announced politely and fade away; errors are
 * announced assertively and stay until dismissed, so an auto-dismiss never hides a failure.
 */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => {
    setItems((current) => current.filter((item) => item.id !== id));
  }, []);

  const show = useCallback(
    (kind: ToastKind, text: string) => {
      const id = nextId.current;
      nextId.current += 1;
      setItems((current) => [...current, { id, kind, text }]);
      if (kind !== 'error') {
        window.setTimeout(() => {
          dismiss(id);
        }, SUCCESS_VISIBLE_MS);
      }
    },
    [dismiss],
  );

  const api = useMemo(() => ({ show }), [show]);
  const polite = items.filter((item) => item.kind !== 'error');
  const assertive = items.filter((item) => item.kind === 'error');

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className={styles.region} data-testid="toast-region">
        <div role="status" aria-live="polite" aria-atomic="false">
          {polite.map((item) => (
            <ToastView key={item.id} item={item} onDismiss={dismiss} />
          ))}
        </div>
        <div role="alert" aria-live="assertive" aria-atomic="false">
          {assertive.map((item) => (
            <ToastView key={item.id} item={item} onDismiss={dismiss} />
          ))}
        </div>
      </div>
    </ToastContext.Provider>
  );
}

function ToastView({ item, onDismiss }: { item: ToastItem; onDismiss: (id: number) => void }) {
  return (
    <div className={cx(styles.toast, styles[item.kind])} data-testid={`toast-${item.kind}`}>
      <span>{item.text}</span>
      <button
        type="button"
        className={styles.close}
        aria-label={t('close')}
        onClick={() => {
          onDismiss(item.id);
        }}
      >
        <span aria-hidden="true">×</span>
      </button>
    </div>
  );
}

export function useToast(): ToastApi {
  const value = useContext(ToastContext);
  if (value === null) {
    throw new Error('useToast must be used inside ToastProvider');
  }
  return value;
}
