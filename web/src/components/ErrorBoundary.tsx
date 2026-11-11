import { cx } from '../styles/cx';
import { Component } from 'react';
import type { ReactNode } from 'react';
import { t } from '../i18n/messages';
import shared from '../styles/shared.module.css';

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  failed: boolean;
}

/** Catches rendering failures and shows a generic message with no technical detail. */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { failed: true };
  }

  override render(): ReactNode {
    if (!this.state.failed) {
      return this.props.children;
    }
    return (
      <div className={cx(shared.card, shared.narrow)} role="alert" data-testid="error-boundary">
        <h1>{t('errorUnexpected')}</h1>
        <button
          type="button"
          className={shared.button}
          onClick={() => {
            window.location.reload();
          }}
        >
          {t('retry')}
        </button>
      </div>
    );
  }
}
