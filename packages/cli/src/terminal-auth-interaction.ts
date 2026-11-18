// Adapted from Pi1.5.2 cli/auth-terminal.ts, cli/auth-login.ts and utils/terminal-question.ts.
// Copyright Mario Zechner. MIT notice: ../UPSTREAM-LICENSE.
import { createInterface } from 'node:readline/promises';
import type { AuthEvent, AuthInteraction, AuthPrompt } from '@earendil-works/pi-ai';
import { terminalText } from './terminal-text.ts';
import { SecretPrompt } from './secret-prompt.ts';
export class TerminalAuthInteraction implements AuthInteraction {
  public readonly signal: AbortSignal;
  readonly #abort: AbortController;
  public constructor(abort: AbortController, signal: AbortSignal = abort.signal) {
    this.#abort = abort;
    this.signal = signal;
  }
  public notify(event: AuthEvent): void {
    if (event.type === 'auth_url') {
      this.#write(`Open this URL to sign in:\n${event.url}\n${event.instructions ?? ''}`);
    } else if (event.type === 'device_code') {
      this.#write(`Open ${event.verificationUri} and enter ${event.userCode}`);
    } else {
      this.#write(event.message);
      if (event.type === 'info') {
        for (const link of event.links ?? []) {
          this.#write(`${link.label ?? ''} ${link.url}`);
        }
      }
    }
  }
  public async prompt(prompt: AuthPrompt): Promise<string> {
    const signal = prompt.signal ? AbortSignal.any([this.signal, prompt.signal]) : this.signal;
    signal.throwIfAborted();
    if (prompt.type === 'select') {
      return this.#select(prompt, signal);
    }
    return prompt.type === 'secret' || prompt.type === 'manual_code'
      ? new SecretPrompt(this.#abort, signal).read(`${prompt.message}: `)
      : this.#question(`${prompt.message}: `, signal);
  }
  async #select(
    prompt: Extract<AuthPrompt, { type: 'select' }>,
    signal: AbortSignal,
  ): Promise<string> {
    if (!prompt.options.length) {
      throw new Error('No authentication choices');
    }
    this.#write(prompt.message);
    prompt.options.forEach((option, index) => this.#write(`${index + 1}. ${option.label}`));
    while (true) {
      const answer = await this.#question(`Select 1-${prompt.options.length}: `, signal);
      const selected = /^\d+$/.test(answer.trim())
        ? prompt.options[Number(answer.trim()) - 1]
        : undefined;
      if (selected) {
        return selected.id;
      }
      this.#write('Enter a listed number.');
    }
  }
  #write(text: string): void {
    process.stderr.write(`${terminalText(text)}\n`);
  }
  async #question(message: string, signal: AbortSignal): Promise<string> {
    const readline = createInterface({ input: process.stdin, output: process.stderr });
    const interrupted = new AbortController();
    const interrupt = (): void => {
      interrupted.abort();
      this.#abort.abort();
    };
    readline.on('SIGINT', interrupt);
    readline.on('close', interrupt);
    try {
      return await readline.question(terminalText(message), {
        signal: AbortSignal.any([signal, interrupted.signal]),
      });
    } finally {
      readline.off('close', interrupt);
      readline.off('SIGINT', interrupt);
      readline.close();
    }
  }
}
