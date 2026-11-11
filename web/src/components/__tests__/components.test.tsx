import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Component, useState } from 'react';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';
import { ConfirmDialog } from '../ConfirmDialog';
import { CopyOnceSecret } from '../CopyOnceSecret';
import { ErrorBoundary } from '../ErrorBoundary';
import { FormField } from '../FormField';
import { PasswordField } from '../PasswordField';
import { ResponsiveTable } from '../ResponsiveTable';
import { ToastProvider, useToast } from '../Toast';
import { PASSWORD_RULES, unmetRules } from '../passwordRules';

describe('FormField', () => {
  it('links the label, hint and error to the control', async () => {
    const { container } = render(
      <FormField label="Email" hint="Work address" error="Enter a valid email address" required>
        {(control) => <input {...control} />}
      </FormField>,
    );
    const input = screen.getByLabelText(/Email/);
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input).toHaveAttribute('aria-required', 'true');
    expect(input).toHaveAccessibleDescription('Work address Enter a valid email address');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('has no description or invalid flag without a hint or error', () => {
    render(<FormField label="Name">{(control) => <input {...control} />}</FormField>);
    const input = screen.getByLabelText('Name');
    expect(input).not.toHaveAttribute('aria-invalid');
    expect(input).not.toHaveAttribute('aria-describedby');
  });
});

describe('password rules', () => {
  it('follows BR-2.1 and BR-2.2', () => {
    expect(unmetRules('Correct-Horse-7').map((rule) => rule.id)).toEqual([]);
    expect(unmetRules('short').map((rule) => rule.id)).toEqual([
      'length',
      'upper',
      'digit',
      'symbol',
    ]);
    expect(unmetRules('ALLUPPER-1234').map((rule) => rule.id)).toEqual(['lower']);
    expect(unmetRules('Aa1-' + 'x'.repeat(125)).length).toBe(1);
    expect(PASSWORD_RULES).toHaveLength(5);
  });

  it('counts a symbol as any character that is not a letter or digit', () => {
    expect(unmetRules('Abcdefgh1 ')).toEqual([]);
    expect(unmetRules('Abcdefgh1é').map((rule) => rule.id)).toEqual(['symbol']);
  });
});

describe('PasswordField', () => {
  function Harness({ showRules }: { showRules: boolean }) {
    const [value, setValue] = useState('');
    return (
      <PasswordField
        label="New password"
        value={value}
        onChange={setValue}
        autoComplete="new-password"
        showRules={showRules}
      />
    );
  }

  it('updates the checklist as the person types and can reveal the password', async () => {
    const user = userEvent.setup();
    const { container } = render(<Harness showRules />);
    const input = screen.getByLabelText(/New password/);
    expect(input).toHaveAttribute('type', 'password');
    expect(screen.getByTestId('password-rule-upper')).toHaveAttribute('data-met', 'false');
    await user.type(input, 'Abcdefghi1-');
    for (const id of ['length', 'upper', 'lower', 'digit', 'symbol']) {
      expect(screen.getByTestId(`password-rule-${id}`)).toHaveAttribute('data-met', 'true');
    }
    await user.click(screen.getByTestId('password-toggle'));
    expect(input).toHaveAttribute('type', 'text');
    expect(screen.getByTestId('password-toggle')).toHaveAttribute('aria-pressed', 'true');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('shows no checklist when not choosing a new password', () => {
    render(<Harness showRules={false} />);
    expect(screen.queryByTestId('password-rules')).not.toBeInTheDocument();
  });
});

describe('CopyOnceSecret', () => {
  it('only lets the person finish after they confirm they saved it', async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn();
    const { container } = render(
      <CopyOnceSecret
        label="Recovery codes"
        secret={['AAAA-1111', 'BBBB-2222']}
        onDismiss={onDismiss}
      />,
    );
    expect(screen.getByTestId('secret-value')).toHaveTextContent('AAAA-1111');
    expect(screen.getByRole('note')).toHaveTextContent(/shown once/i);
    const done = screen.getByTestId('secret-done');
    expect(done).toBeDisabled();
    await user.click(screen.getByTestId('secret-ack'));
    expect(done).toBeEnabled();
    await user.click(done);
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('copies every line and says so', async () => {
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(
      <CopyOnceSecret
        label="Link"
        secret={['https://example.test/x']}
        onDismiss={() => undefined}
      />,
    );
    await user.click(screen.getByTestId('secret-copy'));
    await waitFor(() => {
      expect(screen.getByTestId('secret-copy')).toHaveTextContent('Copied');
    });
    expect(writeText).toHaveBeenCalledWith('https://example.test/x');
  });
});

describe('ConfirmDialog', () => {
  function Harness({ typed }: { typed?: string }) {
    const [open, setOpen] = useState(false);
    const [confirmed, setConfirmed] = useState(false);
    return (
      <>
        <button
          type="button"
          onClick={() => {
            setOpen(true);
          }}
        >
          Open
        </button>
        <p data-testid="result">{confirmed ? 'done' : 'pending'}</p>
        <ConfirmDialog
          open={open}
          title="Deactivate Mia"
          confirmLabel="Deactivate"
          destructive
          {...(typed === undefined ? {} : { requireTyped: typed })}
          onConfirm={() => {
            setConfirmed(true);
            setOpen(false);
          }}
          onCancel={() => {
            setOpen(false);
          }}
        >
          They will be signed out everywhere.
        </ConfirmDialog>
      </>
    );
  }

  it('opens as a labelled modal and closes on cancel', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    expect(screen.getByTestId('confirm-dialog')).not.toHaveAttribute('open');
    await user.click(screen.getByText('Open'));
    const dialog = screen.getByTestId('confirm-dialog');
    expect(dialog).toHaveAttribute('open');
    expect(within(dialog).getByRole('heading', { name: 'Deactivate Mia' })).toBeInTheDocument();
    await user.click(screen.getByTestId('confirm-dialog-cancel'));
    expect(dialog).not.toHaveAttribute('open');
    expect(screen.getByTestId('result')).toHaveTextContent('pending');
  });

  it('closes on Escape without confirming', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByText('Open'));
    screen.getByTestId('confirm-dialog').dispatchEvent(new Event('cancel', { cancelable: true }));
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).not.toHaveAttribute('open');
    });
    expect(screen.getByTestId('result')).toHaveTextContent('pending');
  });

  it('can require typing the exact text before confirming', async () => {
    const user = userEvent.setup();
    render(<Harness typed="mia@example.test" />);
    await user.click(screen.getByText('Open'));
    const confirm = screen.getByTestId('confirm-dialog-confirm');
    expect(confirm).toBeDisabled();
    await user.type(screen.getByTestId('confirm-dialog-typed'), 'mia@example.te');
    expect(confirm).toBeDisabled();
    await user.type(screen.getByTestId('confirm-dialog-typed'), 'st');
    expect(confirm).toBeEnabled();
    await user.click(confirm);
    expect(screen.getByTestId('result')).toHaveTextContent('done');
  });

  it('has no accessibility violations when open', async () => {
    const user = userEvent.setup();
    const { container } = render(<Harness />);
    await user.click(screen.getByText('Open'));
    expect(await axe(container)).toHaveNoViolations();
  });
});

describe('ResponsiveTable', () => {
  const columns = [
    { key: 'name', header: 'Name', render: (row: { name: string; role: string }) => row.name },
    { key: 'role', header: 'Role', render: (row: { name: string; role: string }) => row.role },
  ];

  it('renders a real table with labelled cells', async () => {
    const { container } = render(
      <ResponsiveTable
        caption="Users"
        columns={columns}
        rows={[
          { name: 'Ada', role: 'admin' },
          { name: 'Mia', role: 'member' },
        ]}
        rowKey={(row) => row.name}
      />,
    );
    expect(screen.getByRole('table', { name: 'Users' })).toBeInTheDocument();
    expect(screen.getAllByRole('columnheader')).toHaveLength(2);
    expect(screen.getAllByTestId('table-row')).toHaveLength(2);
    expect(screen.getByText('Mia').closest('td')).toHaveAttribute('data-label', 'Name');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('shows an empty state instead of an empty table', () => {
    render(
      <ResponsiveTable
        caption="Users"
        columns={columns}
        rows={[]}
        rowKey={() => 'x'}
        emptyState="No users"
      />,
    );
    expect(screen.getByTestId('table-empty')).toHaveTextContent('No users');
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
  });
});

describe('Toast', () => {
  function Trigger() {
    const toast = useToast();
    return (
      <>
        <button
          type="button"
          onClick={() => {
            toast.show('success', 'Saved');
          }}
        >
          ok
        </button>
        <button
          type="button"
          onClick={() => {
            toast.show('error', 'Nothing was changed');
          }}
        >
          fail
        </button>
      </>
    );
  }

  it('announces successes politely and errors assertively', async () => {
    const user = userEvent.setup();
    const { container } = render(
      <ToastProvider>
        <Trigger />
      </ToastProvider>,
    );
    await user.click(screen.getByText('ok'));
    await user.click(screen.getByText('fail'));
    expect(within(screen.getByRole('status')).getByText('Saved')).toBeInTheDocument();
    expect(within(screen.getByRole('alert')).getByText('Nothing was changed')).toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('fades successes but never hides an error by itself', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime.bind(vi) });
    render(
      <ToastProvider>
        <Trigger />
      </ToastProvider>,
    );
    await user.click(screen.getByText('ok'));
    await user.click(screen.getByText('fail'));
    vi.advanceTimersByTime(7000);
    await waitFor(() => {
      expect(screen.queryByText('Saved')).not.toBeInTheDocument();
    });
    expect(screen.getByText('Nothing was changed')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Close' }));
    expect(screen.queryByText('Nothing was changed')).not.toBeInTheDocument();
    vi.useRealTimers();
  });
});

describe('ErrorBoundary', () => {
  class Broken extends Component {
    override render(): never {
      throw new Error('internal detail that must not be shown');
    }
  }

  it('shows a generic message with no technical detail', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    render(
      <ErrorBoundary>
        <Broken />
      </ErrorBoundary>,
    );
    const alert = screen.getByTestId('error-boundary');
    expect(alert).toHaveTextContent('Something went wrong');
    expect(alert).not.toHaveTextContent('internal detail');
  });
});
