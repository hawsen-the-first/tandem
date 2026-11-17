import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { renderApp } from '../../../../test/render';
import { RUN_ID, finishedRun, integrationsApi, queuedRun, validation } from './fixtures';

const FACTORY_LIMITS = {
  wall_clock_seconds: 60,
  max_output_bytes: 1_048_576,
  max_file_bytes: 10_485_760,
  max_files_written: 100,
  memory_mebibytes: 512,
  cpu_millicores: 1000,
};

const POLICY = {
  runner_tier: 'in_process',
  tool_allow_list: ['file_read', 'web_fetch'],
  network: { mode: 'none', hosts: [] },
  limits: { ...FACTORY_LIMITS, wall_clock_seconds: 120 },
  version: 3,
};

const VIEW = {
  policy: POLICY,
  factory: { ...POLICY, limits: FACTORY_LIMITS, tool_allow_list: ['file_read'], version: 0 },
  ranges: [
    { field: 'wall_clock_seconds', min: 1, max: 3600, default: 60 },
    { field: 'max_output_bytes', min: 1024, max: 104_857_600, default: 1_048_576 },
    { field: 'max_file_bytes', min: 1024, max: 1_073_741_824, default: 10_485_760 },
    { field: 'max_files_written', min: 1, max: 10_000, default: 100 },
    { field: 'memory_mebibytes', min: 64, max: 16_384, default: 512 },
    { field: 'cpu_millicores', min: 100, max: 16_000, default: 1000 },
  ],
  tools: [
    { id: 'file_read', description: 'Read files in the session folder.', can_be_enabled: true },
    { id: 'file_write', description: 'Write files in the session folder.', can_be_enabled: true },
    { id: 'web_fetch', description: 'Fetch a web page.', can_be_enabled: true },
    { id: 'command_run', description: 'Run a command.', can_be_enabled: false },
  ],
};

async function openPage(extra = {}, view: unknown = VIEW) {
  const api = integrationsApi({ 'GET /api/admin/sandbox-policy': { body: view }, ...extra });
  const result = renderApp('/admin/integrations/sandbox-policy');
  await screen.findByTestId('policy-form');
  return { api, ...result };
}

describe('SandboxPolicyPage (US-2.9)', () => {
  it('shows the policy, the banner, the disabled command tool and each limit range', async () => {
    const { container } = await openPage();
    expect(screen.getByTestId('policy-applies')).toHaveTextContent(
      'Changes apply to runs that start after you save. Runs in progress keep their policy.',
    );
    expect(screen.getByTestId('policy-tier-in_process')).toBeChecked();
    expect(screen.getByTestId('policy-tier-container')).toBeDisabled();
    expect(screen.getByTestId('policy-tool-command_run')).toBeDisabled();
    expect(screen.getByTestId('policy-tool-command_run')).not.toBeChecked();
    expect(screen.getByTestId('policy-tool-command_run')).toHaveAccessibleDescription(
      'Commands are not available in this version',
    );
    expect(screen.getByTestId('policy-tool-web_fetch')).toBeChecked();
    expect(screen.getByTestId('policy-network-none')).toBeChecked();
    expect(screen.getByTestId('policy-limit-wall_clock_seconds-input')).toHaveValue('120');
    expect(screen.getByTestId('policy-limit-wall_clock_seconds-input')).toHaveAccessibleDescription(
      'Default 60 seconds. Allowed: 1 to 3,600 seconds.',
    );
    expect(screen.getByTestId('policy-limits')).toHaveTextContent(
      'Memory and CPU are enforced by the container runner only.',
    );
    expect(screen.getByTestId('policy-save')).toBeDisabled();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('resets a limit to its factory default and saves the whole policy', async () => {
    const { api } = await openPage({
      'PUT /api/admin/sandbox-policy': (request: { body: unknown }) => ({
        body: { ...VIEW, policy: { ...(request.body as object), version: 4 } },
      }),
    });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Reset to default: Time per run' }));
    expect(screen.getByTestId('policy-limit-wall_clock_seconds-input')).toHaveValue('60');
    expect(screen.getByTestId('policy-dirty')).toHaveTextContent('unsaved changes');
    await user.click(screen.getByTestId('policy-tool-file_write'));
    await user.click(screen.getByTestId('policy-network-hosts'));
    await user.type(screen.getByRole('textbox', { name: 'Host 1' }), 'PyPI.org');
    await user.click(screen.getByTestId('policy-save'));
    expect(
      await screen.findByText('Sandbox policy saved and recorded in the audit log.'),
    ).toBeInTheDocument();
    const put = api.calls.find((call) => call.method === 'PUT');
    expect(put?.body).toEqual({
      runner_tier: 'in_process',
      tool_allow_list: ['file_read', 'web_fetch', 'file_write'],
      network: { mode: 'allow_hosts', hosts: ['pypi.org'] },
      limits: FACTORY_LIMITS,
    });
    expect(await screen.findByText('Version 4')).toBeInTheDocument();
    expect(screen.getByTestId('policy-save')).toBeDisabled();
  });

  it('refuses out-of-range limits and bad hosts without calling the server', async () => {
    const { api } = await openPage();
    const user = userEvent.setup();
    const input = screen.getByTestId('policy-limit-memory_mebibytes-input');
    await user.clear(input);
    await user.type(input, '10');
    await user.click(screen.getByTestId('policy-network-hosts'));
    await user.type(screen.getByRole('textbox', { name: 'Host 1' }), 'https://bad');
    await user.click(screen.getByTestId('policy-save'));
    expect(screen.getByTestId('policy-limit-memory_mebibytes-error')).toHaveTextContent(
      'Enter a whole number from 64 to 16,384 MiB',
    );
    expect(screen.getByTestId('policy-hosts-error-0')).toHaveTextContent('Enter a host name');
    expect(api.calls.some((call) => call.method === 'PUT')).toBe(false);
  });

  it('offers the container runner only after a successful runtime check', async () => {
    await openPage({
      'POST /api/admin/runtime/check': { status: 202, body: queuedRun('runtime') },
      [`GET /api/admin/test-runs/${RUN_ID}`]: {
        body: finishedRun('runtime', [
          {
            subject: 'runtime',
            status: 'ok',
            message: 'A container runtime answered.',
            details: { repositories: [], runtime_name: 'Docker', runtime_version: '28.1' },
          },
        ]),
      },
    });
    const user = userEvent.setup();
    expect(screen.getByTestId('policy-tier-container')).toBeDisabled();
    await user.click(screen.getByTestId('policy-check-runtime'));
    expect(await screen.findByText('Found Docker 28.1.')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId('policy-tier-container')).toBeEnabled();
    });
    await user.click(screen.getByTestId('policy-tier-container'));
    expect(screen.getByTestId('policy-tier-container')).toBeChecked();
  });

  it('keeps the container runner unavailable when no runtime is reachable', async () => {
    await openPage({
      'POST /api/admin/runtime/check': { status: 202, body: queuedRun('runtime') },
      [`GET /api/admin/test-runs/${RUN_ID}`]: {
        body: finishedRun('runtime', [
          {
            subject: 'runtime',
            status: 'unreachable',
            message: 'No container runtime answered.',
            details: { repositories: [] },
          },
        ]),
      },
    });
    const user = userEvent.setup();
    await user.click(screen.getByTestId('policy-check-runtime'));
    expect(await screen.findByTestId('policy-runtime-unavailable')).toHaveTextContent(
      'container runner is unavailable',
    );
    expect(screen.getByTestId('policy-tier-container')).toBeDisabled();
  });

  it('shows a server refusal beside the field it names', async () => {
    await openPage({
      'PUT /api/admin/sandbox-policy': validation(
        'tool_allow_list',
        'command_run cannot be enabled',
      ),
    });
    const user = userEvent.setup();
    await user.click(screen.getByTestId('policy-tool-file_write'));
    await user.click(screen.getByTestId('policy-save'));
    expect(
      await within(screen.getByTestId('policy-tools')).findByText('command_run cannot be enabled'),
    ).toBeInTheDocument();
    expect(screen.getByTestId('policy-error')).toBeInTheDocument();
  });

  it('is operable by keyboard alone', async () => {
    await openPage();
    const user = userEvent.setup();
    screen.getByTestId('policy-tool-file_write').focus();
    await user.keyboard(' ');
    expect(screen.getByTestId('policy-tool-file_write')).toBeChecked();
    screen.getByTestId('policy-network-none').focus();
    await user.keyboard('{ArrowDown}');
    expect(screen.getByTestId('policy-network-hosts')).toBeChecked();
  });

  it('has no accessibility violations with errors showing', async () => {
    const { container } = await openPage();
    const user = userEvent.setup();
    await user.clear(screen.getByTestId('policy-limit-cpu_millicores-input'));
    await user.click(screen.getByTestId('policy-save'));
    expect(await axe(container)).toHaveNoViolations();
  });
});
