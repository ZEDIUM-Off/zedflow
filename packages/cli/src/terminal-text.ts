// Adapted from Pi1.5.2 cli/auth-terminal.ts, cli/auth-login.ts and utils/terminal-question.ts.
// Copyright Mario Zechner. MIT notice: ../UPSTREAM-LICENSE.
export const terminalText = (text: string): string =>
  Array.from(text)
    .filter((char) => {
      const code = char.charCodeAt(0);
      return char === '\n' || char === '\t' || (code >= 32 && (code < 127 || code > 159));
    })
    .join('');
