import {
  AuthPanel,
  SecretInput,
  StateView,
  TranscriptView,
} from '../../../packages/tui/src/index.ts';

export const wideTranscriptView = (wide: string): TranscriptView => {
  const transcript = new TranscriptView();
  transcript.setMessages([
    { role: 'user', content: wide },
    {
      role: 'assistant',
      content: [
        { type: 'text', text: wide },
        { type: 'thinking', thinking: wide },
      ],
    },
    { role: 'toolResult', toolName: wide, isError: false, content: [{ type: 'text', text: wide }] },
  ]);
  transcript.setStreaming({ role: 'assistant', content: [{ type: 'text', text: wide }] });
  return transcript;
};

export const wideStateView = (wide: string): StateView => {
  const state = new StateView();
  state.setView({
    value: { nested: wide },
    status: 'active',
    context: { wide, list: [wide, wide] },
  });
  return state;
};

export const wideSecretInput = (wide: string): SecretInput => {
  const secret = new SecretInput();
  secret.focused = true;
  for (const character of wide) {
    secret.handleInput(character);
  }
  return secret;
};

export const wideAuthPanel = (wide: string): AuthPanel => {
  const panel = new AuthPanel({ requestRender: () => {}, onCancel: () => {} });
  panel.setTitle(wide);
  panel.show({
    type: 'auth_url',
    url: `https://example.test/${'q'.repeat(300)}`,
    instructions: wide,
  });
  panel.show({
    type: 'device_code',
    userCode: 'ABCD-1234',
    verificationUri: 'https://example.test/device',
  });
  panel.show({
    type: 'info',
    message: wide,
    links: [{ url: 'https://example.test/x', label: wide }],
  });
  return panel;
};
