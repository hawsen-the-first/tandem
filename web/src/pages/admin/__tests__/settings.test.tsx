import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../../test/render';
import { validateSettings } from '../SettingsPage';
import { adminApi } from './fixtures';

const DEFAULTS = {
  idle_timeout_seconds: 12 * 3600,
  absolute_duration_seconds: 14 * 24 * 3600,
  online_breach_check: false,
};

async function openPage(extra = {}) {
  const api = adminApi({ 'GET /api/admin/settings': { body: DEFAULTS }, ...extra });
  const view = renderApp('/admin/settings');
  await screen.findByTestId('settings-form');
  return { api, ...view };
}

async function setField(testId: string, value: string) {
  const user = userEvent.setup();
  const input = screen.getByTestId(testId);
  await user.clear(input);
  if (value !== '') {
    await user.type(input, value);
  }
  return user;
}

describe('validateSettings', () => {
  const ok = { idleMinutes: '720', absoluteHours: '336', onlineCheck: false };
  it('accepts the defaults and the range ends', () => {
    expect(validateSettings(ok)).toEqual({});
    expect(validateSettings({ ...ok, idleMinutes: '15', absoluteHours: '1' })).toEqual({});
    expect(validateSettings({ ...ok, idleMinutes: '43200', absoluteHours: '2160' })).toEqual({});
  });
  it('refuses values outside BR-10.1 or that are not whole numbers', () => {
    expect(validateSettings({ ...ok, idleMinutes: '14' }).idle).toContain('15 to 43,200');
    expect(validateSettings({ ...ok, idleMinutes: '43201' }).idle).toBeDefined();
    expect(validateSettings({ ...ok, absoluteHours: '0' }).absolute).toContain('1 to 2,160');
    expect(validateSettings({ ...ok, absoluteHours: '2161' }).absolute).toBeDefined();
    expect(validateSettings({ ...ok, idleMinutes: '1.5' }).idle).toBeDefined();
    expect(validateSettings({ ...ok, absoluteHours: '' }).absolute).toBeDefined();
  });
  it('refuses an idle time longer than the total', () => {
    expect(validateSettings({ ...ok, idleMinutes: '600', absoluteHours: '5' }).idle).toBe(
      'The idle time cannot be longer than the total sign-in time',
    );
  });
});

describe('SettingsPage', () => {
  it('shows the stored values in friendly units with their ranges and defaults', async () => {
    const { container } = await openPage();
    expect(screen.getByTestId('settings-idle-input')).toHaveValue('720');
    expect(screen.getByTestId('settings-absolute-input')).toHaveValue('336');
    expect(screen.getByTestId('settings-online-input')).not.toBeChecked();
    expect(screen.getByTestId('settings-idle-input')).toHaveAccessibleDescription(
      /Default 12 hours.*15 minutes to 30 days/,
    );
    expect(screen.getByTestId('settings-absolute-input')).toHaveAccessibleDescription(
      /Default 14 days.*1 hour to 90 days/,
    );
    expect(screen.getByText(/apply to new sign-ins/)).toBeInTheDocument();
    expect(screen.getByTestId('settings-save')).toBeDisabled();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('shows a loading state and a load error with retry', async () => {
    const api = adminApi({ 'GET /api/admin/settings': { status: 500, body: 'x' } });
    const user = userEvent.setup();
    renderApp('/admin/settings');
    const error = await screen.findByTestId('settings-load-error');
    api.on('GET /api/admin/settings', { body: DEFAULTS });
    await user.click(within(error).getByRole('button', { name: 'Try again' }));
    expect(await screen.findByTestId('settings-form')).toBeInTheDocument();
  });

  it('saves in seconds and tells the person it applies to new sign-ins', async () => {
    const { api } = await openPage({
      'PUT /api/admin/settings': (request: { body: unknown }) => ({ body: request.body }),
    });
    await setField('settings-idle-input', '60');
    const user = await setField('settings-absolute-input', '24');
    await user.click(screen.getByTestId('settings-online-input'));
    expect(screen.getByTestId('settings-save')).toBeEnabled();
    await user.click(screen.getByTestId('settings-save'));
    expect(
      await screen.findByText('Settings saved. They apply to new sign-ins.'),
    ).toBeInTheDocument();
    const put = api.calls.find((call) => call.method === 'PUT');
    expect(put?.body).toEqual({
      idle_timeout_seconds: 3600,
      absolute_duration_seconds: 86400,
      online_breach_check: true,
    });
    expect(put?.headers.get('x-csrf-token')).toBe('csrf-admin');
    expect(screen.getByTestId('settings-save')).toBeDisabled();
  });

  it('refuses out-of-range values without calling the server', async () => {
    const { api } = await openPage();
    await setField('settings-idle-input', '5');
    const user = await setField('settings-absolute-input', '3000');
    await user.click(screen.getByTestId('settings-save'));
    expect(screen.getByTestId('settings-idle-error')).toHaveTextContent('15 to 43,200');
    expect(screen.getByTestId('settings-absolute-error')).toHaveTextContent('1 to 2,160');
    expect(screen.getByTestId('settings-idle-input')).toHaveAttribute('aria-invalid', 'true');
    expect(api.calls.some((call) => call.method === 'PUT')).toBe(false);
  });

  it('refuses an idle time longer than the total', async () => {
    const { api } = await openPage();
    await setField('settings-idle-input', '600');
    const user = await setField('settings-absolute-input', '5');
    await user.click(screen.getByTestId('settings-save'));
    expect(screen.getByTestId('settings-idle-error')).toHaveTextContent(
      'cannot be longer than the total',
    );
    expect(api.calls.some((call) => call.method === 'PUT')).toBe(false);
  });

  it('shows a server refusal and keeps the edits', async () => {
    await openPage({
      'PUT /api/admin/settings': {
        status: 422,
        body: {
          error: {
            code: 'validation',
            message: 'Check the highlighted fields.',
            fields: [
              {
                field: 'idle_timeout',
                message: 'The idle timeout must be between 15 minutes and 30 days',
              },
            ],
          },
        },
      },
    });
    const user = await setField('settings-idle-input', '30');
    await user.click(screen.getByTestId('settings-save'));
    expect(await screen.findByTestId('settings-error')).toHaveTextContent(
      'between 15 minutes and 30 days',
    );
    expect(screen.getByTestId('settings-idle-input')).toHaveValue('30');
  });

  it('is operable by keyboard alone', async () => {
    const { api } = await openPage({
      'PUT /api/admin/settings': (request: { body: unknown }) => ({ body: request.body }),
    });
    const user = userEvent.setup();
    screen.getByTestId('settings-idle-input').focus();
    await user.keyboard('{Control>}a{/Control}90');
    await user.tab();
    expect(screen.getByTestId('settings-absolute-input')).toHaveFocus();
    await user.tab();
    expect(screen.getByTestId('settings-online-input')).toHaveFocus();
    await user.keyboard(' ');
    expect(screen.getByTestId('settings-online-input')).toBeChecked();
    await user.tab();
    expect(screen.getByTestId('settings-save')).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(await screen.findByText(/Settings saved/)).toBeInTheDocument();
    expect(api.calls.find((call) => call.method === 'PUT')?.body).toMatchObject({
      idle_timeout_seconds: 5400,
      online_breach_check: true,
    });
  });

  it('has no accessibility violations with errors showing', async () => {
    const { container } = await openPage();
    const user = await setField('settings-idle-input', '1');
    await user.click(screen.getByTestId('settings-save'));
    expect(await axe(container)).toHaveNoViolations();
  });
});
