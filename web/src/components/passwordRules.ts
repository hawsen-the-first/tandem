import type { MessageKey } from '../i18n/messages';

export interface PasswordRule {
  id: 'length' | 'upper' | 'lower' | 'digit' | 'symbol';
  label: MessageKey;
  met: (password: string) => boolean;
}

// BR-2.1 and BR-2.2. The server always re-validates; this only guides the person typing.
export const PASSWORD_RULES: PasswordRule[] = [
  {
    id: 'length',
    label: 'passwordRuleLength',
    met: (password) => Array.from(password).length >= 10 && Array.from(password).length <= 128,
  },
  { id: 'upper', label: 'passwordRuleUpper', met: (password) => /\p{Lu}/u.test(password) },
  { id: 'lower', label: 'passwordRuleLower', met: (password) => /\p{Ll}/u.test(password) },
  { id: 'digit', label: 'passwordRuleDigit', met: (password) => /\p{Nd}/u.test(password) },
  {
    id: 'symbol',
    label: 'passwordRuleSymbol',
    met: (password) => /[^\p{L}\p{Nd}]/u.test(password),
  },
];

export function unmetRules(password: string): PasswordRule[] {
  return PASSWORD_RULES.filter((rule) => !rule.met(password));
}
