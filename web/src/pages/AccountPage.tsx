import { publicMessages as p } from '../i18n/messages.public';
import { ChangePasswordForm } from './account/ChangePasswordForm';
import { MfaSection } from './account/MfaSection';

export function Component() {
  return (
    <div data-testid="account-page">
      <h1>{p.accountTitle}</h1>
      <ChangePasswordForm />
      <MfaSection />
    </div>
  );
}
