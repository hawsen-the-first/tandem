import { useEffect, useRef } from 'react';
import type { ReactNode, RefObject } from 'react';
import { cx } from '../styles/cx';
import shared from '../styles/shared.module.css';

const EMAIL_SHAPE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function emailProblem(email: string, message: string): string | undefined {
  const trimmed = email.trim();
  return trimmed.length === 0 || trimmed.length > 254 || !EMAIL_SHAPE.test(trimmed)
    ? message
    : undefined;
}

export function nameProblem(name: string, message: string): string | undefined {
  const length = Array.from(name.trim()).length;
  return length < 1 || length > 80 ? message : undefined;
}

export function totpProblem(code: string, message: string): string | undefined {
  return /^\d{6}$/.test(code.trim()) ? undefined : message;
}

/** The one place a page shows an overall failure. It is announced and receives focus on failure. */
export function FormError({ message, testId }: { message: string | null; testId: string }) {
  if (message === null) {
    return null;
  }
  return (
    <div
      role="alert"
      tabIndex={-1}
      data-form-error
      data-testid={testId}
      className={cx(shared.banner, shared.bannerError)}
    >
      {message}
    </div>
  );
}

/**
 * After a failed submit (`signal` goes up), move focus to the first invalid field, or to the
 * overall error when no field is at fault.
 */
export function useFailureFocus(container: RefObject<HTMLElement | null>, signal: number): void {
  const last = useRef(0);
  useEffect(() => {
    if (signal === 0 || signal === last.current) {
      return;
    }
    last.current = signal;
    const root = container.current;
    const target =
      root?.querySelector<HTMLElement>('[aria-invalid="true"]') ??
      root?.querySelector<HTMLElement>('[data-form-error]');
    target?.focus();
  }, [container, signal]);
}

/** A page in the plain centred card used before sign-in. */
export function AuthCard({
  title,
  children,
  testId,
}: {
  title: string;
  children: ReactNode;
  testId: string;
}) {
  return (
    <main className={cx(shared.card, shared.narrow)} data-testid={testId}>
      <h1>{title}</h1>
      {children}
    </main>
  );
}
