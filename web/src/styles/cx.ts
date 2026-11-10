/** Join class names, skipping anything missing (CSS-module lookups are `string | undefined`). */
export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter((part): part is string => typeof part === 'string' && part !== '').join(' ');
}
