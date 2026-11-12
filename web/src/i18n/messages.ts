// All user-facing text lives here so it can be translated later. Wording for errors follows
// business-rules.md (Error handling); no message ever echoes a password or token.

export const messages = {
  appName: 'Tandem',
  loading: 'Loading…',
  skipToContent: 'Skip to content',
  retry: 'Try again',
  cancel: 'Cancel',
  confirm: 'Confirm',
  close: 'Close',
  copy: 'Copy',
  copied: 'Copied',

  errorUnexpected: 'Something went wrong. Nothing was changed.',
  errorNetwork: 'Could not reach Tandem. Check your connection and try again.',
  errorNotFound: 'Not found',
  errorForbidden: 'You do not have access',
  errorUnauthenticated: 'You need to sign in',
  errorInvalidCredentials: 'That email and password do not match',
  errorInvalidLink: 'This link is not valid. Ask an admin for a new one',
  errorBusy: 'Tandem is busy right now. Try again in a moment',
  errorRefresh: 'Your session needs refreshing. Reload the page and try again',
  errorTooManyConnections: 'Too many open connections. Close another tab and try again',
  errorLastAdmin: 'At least one active admin is required',

  pageNotFoundTitle: 'Page not found',
  pageNotFoundBody: 'That page does not exist, or you do not have access to it.',
  goHome: 'Go to the home page',

  themeLabel: 'Theme',
  themeLight: 'Light',
  themeDark: 'Dark',
  themeSystem: 'System',

  navMenu: 'Menu',
  navChats: 'Chats',
  navAccount: 'Account',
  navUsers: 'Users',
  navSettings: 'Settings',
  navAudit: 'Audit log',
  navPrimary: 'Main navigation',
  signOut: 'Sign out',
  userMenu: 'Account menu',

  passwordShow: 'Show password',
  passwordHide: 'Hide password',
  passwordRulesTitle: 'Your password needs',
  passwordRuleLength: '10 to 128 characters',
  passwordRuleUpper: 'An upper-case letter',
  passwordRuleLower: 'A lower-case letter',
  passwordRuleDigit: 'A digit',
  passwordRuleSymbol: 'A symbol (not a letter or digit)',
  passwordRuleMet: 'met',
  passwordRuleUnmet: 'not met yet',
  passwordsDoNotMatch: 'Passwords do not match',

  secretShownOnce: 'Shown once. Copy it now: Tandem cannot show it again.',
  secretSaved: 'I have saved this',
  secretDone: 'Done',

  tableEmpty: 'Nothing to show yet',

  validationEmail: 'Enter a valid email address',
  validationName: 'Enter a name up to 80 characters',
  validationTotp: 'Enter the 6-digit code',
  validationRecovery: 'Enter one of your recovery codes',
  validationRequired: 'This field is required',

  footerLabel: 'Legal notices',
  footerWarranty:
    'This program comes with ABSOLUTELY NO WARRANTY. It is free software: you may redistribute and modify it under the terms of the GNU Affero General Public License, version 3.',
  footerViewLicense: 'View the licence',
  footerSource: 'Source code',
  footerBasedOn: 'Based on',
} as const;

export type MessageKey = keyof typeof messages;

/** Replace `{name}` placeholders in a message from `values`. Area-specific catalogues use this too. */
export function format(template: string, values?: Record<string, string | number>): string {
  let text = template;
  if (values !== undefined) {
    for (const [name, value] of Object.entries(values)) {
      text = text.replaceAll(`{${name}}`, String(value));
    }
  }
  return text;
}

/** Look up a shared message. */
export function t(key: MessageKey, values?: Record<string, string | number>): string {
  return format(messages[key], values);
}

/** "Too many attempts. Try again in N seconds": one wording for delay, lock and rate limit. */
export function throttleMessage(seconds: number): string {
  return seconds === 1
    ? 'Too many attempts. Try again in 1 second'
    : `Too many attempts. Try again in ${String(seconds)} seconds`;
}
