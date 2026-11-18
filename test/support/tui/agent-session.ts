import {
  AgentControls,
  agentMachine,
  assistant,
  versioned,
} from '../../../packages/tui/test/fixtures.ts';
import { Session } from './session.ts';

/** Owns observer wiring for a real fake-agent actor and its native terminal session. */
export class AgentSession {
  readonly #session: Session;
  readonly #controls: AgentControls;

  private constructor(session: Session, controls: AgentControls) {
    this.#session = session;
    this.#controls = controls;
    controls.observeModel = (event) => session.ui.modelEvent(event);
    controls.observeTool = (result) => session.ui.toolUpdate(result);
  }

  public static async open(
    options: { readonly columns?: number; readonly rows?: number; readonly refuse?: boolean } = {},
  ): Promise<AgentSession> {
    const controls = new AgentControls();
    const session = await Session.open(
      'agent',
      options.refuse ? versioned(controls) : () => agentMachine(controls),
      options,
    );
    return new AgentSession(session, controls);
  }

  public get session(): Session {
    return this.#session;
  }
  public get controls(): AgentControls {
    return this.#controls;
  }

  /** Advance this agent's native model gate into an observed lookup tool call. */
  public async startLookup(): Promise<string> {
    this.#controls.finishModel(
      assistant(
        [
          { type: 'text', text: 'checking' },
          { type: 'toolCall', id: 'call-1', name: 'lookup', arguments: { query: 'q' } },
        ],
        'toolUse',
      ),
    );
    await this.#session.screen.waitFor('state: tool');
    this.#controls.observeTool({
      role: 'toolResult',
      toolCallId: 'call-1',
      toolName: 'lookup',
      content: [{ type: 'text', text: 'tool progress text' }],
      isError: false,
      timestamp: 0,
    });
    return await this.#session.screen.waitFor('tool progress text');
  }

  /** Settle the tool gate, then the following native model gate. */
  public async finishLookup(): Promise<string> {
    this.#controls.finishTool({
      role: 'toolResult',
      toolCallId: 'call-1',
      toolName: 'lookup',
      content: [{ type: 'text', text: 'tool final text' }],
      isError: false,
      timestamp: 0,
    });
    await this.#session.screen.waitFor('state: thinking');
    this.#controls.finishModel(assistant([{ type: 'text', text: 'final answer' }]));
    return await this.#session.screen.waitFor('state: ready');
  }

  public async beginPrompt(text: string): Promise<void> {
    this.#session.screen.submit(text);
    await this.#session.screen.waitFor('state: thinking');
  }
}
