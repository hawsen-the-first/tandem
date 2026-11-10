import { createHmac } from 'node:crypto';

const BASE32 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

function decodeBase32(text: string): Buffer {
  let bits = 0;
  let value = 0;
  const bytes: number[] = [];
  for (const character of text.replace(/=+$/, '').toUpperCase()) {
    const index = BASE32.indexOf(character);
    if (index < 0) {
      throw new Error('not base32');
    }
    value = (value << 5) | index;
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((value >>> bits) & 0xff);
    }
  }
  return Buffer.from(bytes);
}

/** The 6-digit TOTP code (RFC 6238, SHA-1, 30 s) for a base32 secret at a given time. */
export function totpCode(secretBase32: string, atMillis: number = Date.now()): string {
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(atMillis / 1000 / 30)));
  const digest = createHmac('sha1', decodeBase32(secretBase32)).update(counter).digest();
  const offset = (digest[digest.length - 1] ?? 0) & 0x0f;
  const truncated =
    (((digest[offset] ?? 0) & 0x7f) << 24) |
    ((digest[offset + 1] ?? 0) << 16) |
    ((digest[offset + 2] ?? 0) << 8) |
    (digest[offset + 3] ?? 0);
  return String(truncated % 1_000_000).padStart(6, '0');
}
