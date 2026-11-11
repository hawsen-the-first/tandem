import { t } from '../i18n/messages';
import type { ThemeChoice } from './ThemeProvider';
import { useTheme } from './ThemeProvider';

const CHOICES: { value: ThemeChoice; label: 'themeLight' | 'themeDark' | 'themeSystem' }[] = [
  { value: 'system', label: 'themeSystem' },
  { value: 'light', label: 'themeLight' },
  { value: 'dark', label: 'themeDark' },
];

export function ThemeToggle() {
  const { choice, setChoice } = useTheme();
  return (
    <label>
      <span className="visually-hidden">{t('themeLabel')}</span>
      <select
        data-testid="theme-toggle"
        value={choice}
        onChange={(event) => {
          setChoice(event.target.value as ThemeChoice);
        }}
      >
        {CHOICES.map((item) => (
          <option key={item.value} value={item.value}>
            {t(item.label)}
          </option>
        ))}
      </select>
    </label>
  );
}
