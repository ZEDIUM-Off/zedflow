import type { AuthType, LoginOptions, Models, Provider } from '@earendil-works/pi-ai';
import { chooseProvider } from './auth-selection.ts';
import { sanitizeInline } from './text.ts';
import { TerminalLoginAttempt } from './terminal-login-attempt.ts';
import { selectLogin, selectLoginMethod } from './terminal-login-selection.ts';
import type { TerminalNotices } from './terminal-notices.ts';
import type { TerminalOperationScope } from './terminal-operation-scope.ts';
import type { TerminalPanelSlot } from './terminal-panel-slot.ts';

const AUTH_LISTED_PROVIDERS = 10;

/** Serializes interactive login and owns the native models and their login configuration. */
export class TerminalLogin {
  readonly #models: Models;
  readonly #loginOptions: LoginOptions | undefined;
  readonly #notices: TerminalNotices;
  readonly #slot: TerminalPanelSlot;
  #pending: TerminalLoginAttempt | undefined;

  public constructor(
    models: Models,
    loginOptions: LoginOptions | undefined,
    notices: TerminalNotices,
    slot: TerminalPanelSlot,
  ) {
    this.#models = models;
    this.#loginOptions = loginOptions;
    this.#notices = notices;
    this.#slot = slot;
  }

  public get active(): boolean {
    return this.#pending !== undefined;
  }

  public cancel(): void {
    this.#pending?.cancel();
  }

  public async signIn(args: readonly string[], scope: TerminalOperationScope): Promise<void> {
    if (this.#pending) {
      this.#notices.add('A login is already in progress.');
      return;
    }
    const selection = selectLogin(this.#models, args);
    if (typeof selection === 'string') {
      this.#notices.add(selection);
      return;
    }
    await this.#runLogin(selection.provider, selection.requested, scope);
  }

  async #runLogin(
    provider: Provider,
    requested: AuthType | undefined,
    scope: TerminalOperationScope,
  ): Promise<void> {
    const attempt = new TerminalLoginAttempt(scope, this.#slot);
    this.#pending = attempt;
    const interaction = attempt.open(provider.name);
    try {
      const method = requested ?? (await selectLoginMethod(provider, interaction));
      // The native flow persists its credential; the returned value is not retained.
      await this.#models.login(provider.id, method, interaction, this.#loginOptions);
      this.#notices.add(`Signed in to ${sanitizeInline(provider.name, 60)} (${method}).`);
    } catch {
      this.#notices.add(
        attempt.cancelled
          ? 'Login cancelled.'
          : 'Login failed. Check the authentication method, network and credential storage.',
      );
    } finally {
      attempt.close();
      this.#pending = undefined;
      this.#slot.hide();
    }
  }

  public async signOut(args: readonly string[], scope: TerminalOperationScope): Promise<void> {
    const choice = chooseProvider(this.#models, args[0]);
    if ('problem' in choice) {
      this.#notices.add(`${choice.problem}.`);
      return;
    }
    const controller = scope.controller();
    try {
      await this.#models.logout(choice.provider.id, { signal: controller.signal });
      this.#notices.add(`Signed out of ${sanitizeInline(choice.provider.name, 60)}.`);
    } finally {
      scope.release(controller);
    }
  }

  public async status(scope: TerminalOperationScope): Promise<void> {
    const controller = scope.controller();
    try {
      for (const provider of this.#models.getProviders().slice(0, AUTH_LISTED_PROVIDERS)) {
        const check = await this.#models.checkAuth(provider.id, { signal: controller.signal });
        const state = check
          ? `signed in (${check.type}${check.source ? `, ${sanitizeInline(check.source, 80)}` : ''})`
          : 'not signed in';
        this.#notices.add(`${sanitizeInline(provider.name, 60)}: ${state}`);
      }
    } finally {
      scope.release(controller);
    }
  }
}
