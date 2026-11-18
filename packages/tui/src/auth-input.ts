import { Input, SelectList } from '@earendil-works/pi-tui';
import type { AuthPrompt } from '@earendil-works/pi-ai';
import { SecretInput } from './secret-input.ts';
import { selectListTheme } from './style.ts';
import { sanitizeInline } from './text.ts';

const SELECT_ROWS = 7;

const createAuthSelectInput = (
  options: Extract<AuthPrompt, { type: 'select' }>['options'],
  submit: (value: string) => void,
  cancel: () => void,
): SelectList => {
  const list = new SelectList(
    options.map((option) => ({
      value: option.id,
      label: sanitizeInline(option.label, 80),
      ...(option.description ? { description: sanitizeInline(option.description, 120) } : {}),
    })),
    SELECT_ROWS,
    selectListTheme,
  );
  list.onSelect = (item) => submit(item.value);
  list.onCancel = cancel;
  return list;
};

export const createAuthInput = (
  prompt: AuthPrompt,
  submit: (value: string) => void,
  cancel: () => void,
): SecretInput | Input | SelectList => {
  if (prompt.type === 'select') {
    return createAuthSelectInput(prompt.options, submit, cancel);
  }
  if (prompt.type === 'text') {
    const input = new Input();
    input.onSubmit = submit;
    input.onEscape = cancel;
    return input;
  }
  // Secrets and pasted OAuth codes or redirect URLs are never displayed.
  const secret = new SecretInput();
  secret.onSubmit = submit;
  secret.onCancel = cancel;
  return secret;
};
