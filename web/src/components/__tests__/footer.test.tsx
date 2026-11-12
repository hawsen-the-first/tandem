import { screen } from '@testing-library/react';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import { ABOUT, UNAUTHENTICATED, mockApi } from '../../test/mockApi';
import { renderApp } from '../../test/render';

describe('legal notices footer', () => {
  it('shows the copyright, no warranty, the licence, the source of this version and the credit', async () => {
    mockApi({ 'GET /api/auth/me': UNAUTHENTICATED });
    const { container } = renderApp('/sign-in');
    const footer = await screen.findByTestId('site-footer');
    expect(footer).toHaveTextContent('Copyright (C) 2026 Reuben Hawthorne-Jensen');
    expect(footer).toHaveTextContent('ABSOLUTELY NO WARRANTY');
    expect(footer).toHaveTextContent('GNU Affero General Public License');
    expect(await screen.findByTestId('footer-license')).toHaveAttribute('href', ABOUT.license_url);
    expect(await screen.findByTestId('footer-source')).toHaveAttribute('href', ABOUT.source_url);
    expect(screen.getByTestId('footer-upstream')).toHaveAttribute('href', ABOUT.upstream_url);
    expect(footer).toHaveTextContent('Based on Tandem');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('still shows the notices when the details cannot be loaded', async () => {
    const api = mockApi({ 'GET /api/auth/me': UNAUTHENTICATED });
    api.on('GET /api/about', { status: 500, body: { error: { code: 'internal', message: 'x' } } });
    renderApp('/sign-in');
    const footer = await screen.findByTestId('site-footer');
    expect(footer).toHaveTextContent('ABSOLUTELY NO WARRANTY');
    expect(footer).toHaveTextContent('Based on Tandem');
    expect(screen.getByTestId('footer-license')).toBeInTheDocument();
  });

  it('is on signed-in pages too', async () => {
    mockApi({ 'GET /api/auth/me': { body: (await import('../../test/mockApi')).ME_ADMIN } });
    renderApp('/');
    expect(await screen.findByTestId('site-footer')).toBeInTheDocument();
  });
});
