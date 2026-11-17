import { ti } from '../../i18n/messages.integrations';

// Client-side mirrors of BR-19 (outbound URLs) and BR-18.4 (network hosts). They catch typing
// mistakes early; the server always decides and its reasons win.

function ipv4(host: string): number[] | null {
  const parts = host.split('.');
  if (parts.length !== 4 || !parts.every((part) => /^\d{1,3}$/.test(part))) {
    return null;
  }
  const octets = parts.map(Number);
  return octets.every((octet) => octet <= 255) ? octets : null;
}

function isBlocked(host: string): boolean {
  const v4 = ipv4(host);
  if (v4 !== null) {
    return v4[0] === 169 && v4[1] === 254;
  }
  const v6 = host.replace(/^\[|\]$/g, '').toLowerCase();
  return /^fe[89ab][0-9a-f]:/.test(v6) || v6 === 'fd00:ec2::254';
}

function isLocalOrPrivate(host: string): boolean {
  const name = host.toLowerCase();
  if (name.startsWith('[')) {
    // IPv6 literals: the server classifies them; do not refuse here.
    return true;
  }
  const v4 = ipv4(name);
  if (v4 !== null) {
    const [a = 0, b = 0] = v4;
    return a === 127 || a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168);
  }
  return (
    !name.includes('.') ||
    name === 'localhost' ||
    name.endsWith('.localhost') ||
    name.endsWith('.local') ||
    name.endsWith('.internal')
  );
}

/** The BR-19 reason a base URL would be refused, or null when it looks acceptable. */
export function checkOutboundUrl(text: string): string | null {
  const value = text.trim();
  if (value === '') {
    return ti('urlRequired');
  }
  if (!/^[a-z][a-z0-9+.-]*:/i.test(value)) {
    return ti('urlScheme');
  }
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return ti('urlInvalid');
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    return ti('urlScheme');
  }
  if (url.hostname === '') {
    return ti('urlInvalid');
  }
  if (url.username !== '' || url.password !== '') {
    return ti('urlCredentials');
  }
  if (value.includes('?') || value.includes('#')) {
    return ti('urlQuery');
  }
  if (isBlocked(url.hostname)) {
    return ti('urlBlocked');
  }
  if (url.protocol === 'http:' && !isLocalOrPrivate(url.hostname)) {
    return ti('urlPlainHttp');
  }
  return null;
}

export const MAX_HOSTS = 50;
const HOST_PATTERN =
  /^(\*\.)?([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)*$/i;

/** Problems in a network allow-list, keyed by row index (BR-18.4). */
export function checkHosts(hosts: string[]): Record<number, string> {
  const problems: Record<number, string> = {};
  const seen = new Set<string>();
  hosts.forEach((raw, index) => {
    const host = raw.trim().toLowerCase();
    if (host === '' || host.length > 253 || !HOST_PATTERN.test(host)) {
      problems[index] = ti('hostInvalid');
    } else if (seen.has(host)) {
      problems[index] = ti('hostDuplicate');
    }
    seen.add(host);
  });
  return problems;
}
