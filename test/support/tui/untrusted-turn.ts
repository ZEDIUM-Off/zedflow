import { assistant, textEvent } from '../../../packages/tui/test/fixtures.ts';
import type { AgentSession } from './agent-session.ts';

/** Owns one payload so the run prompt, model output and tool output carry identical untrusted text. */
export class UntrustedTurn {
  readonly #agent: AgentSession;
  readonly #hostile =
    'safe\u001b]0;PWNED-TITLE\u0007\u001b[2J\u001b[9999;9999HINJECTED\u009b31m\r\ncarriage\u0007bell\u202Eevil\u001b_Gimage\u001b\\end';

  public constructor(agent: AgentSession) {
    this.#agent = agent;
  }

  public async requestTool(): Promise<void> {
    // Typed characters would be interpreted as keys, so deliver the hostile prompt natively.
    this.#agent.session.run.runs.send(this.#agent.session.run.runId, {
      type: 'PROMPT',
      text: this.#hostile,
    });
    await this.#agent.session.screen.waitFor('state: thinking');
    this.#agent.controls.observeModel(textEvent(this.#hostile));
    this.#agent.controls.finishModel(
      assistant(
        [
          { type: 'text', text: this.#hostile },
          { type: 'thinking', thinking: this.#hostile },
          {
            type: 'toolCall',
            id: 'c',
            name: `tool${this.#hostile}`,
            arguments: { value: this.#hostile },
          },
        ],
        'toolUse',
      ),
    );
    await this.#agent.session.screen.waitFor('state: tool');
  }

  public async completeTool(): Promise<string> {
    this.#agent.controls.observeTool({
      role: 'toolResult',
      toolCallId: 'c',
      toolName: this.#hostile,
      content: [{ type: 'text', text: this.#hostile }],
      isError: false,
      timestamp: 0,
    });
    await this.#agent.session.terminal.screen();
    this.#agent.controls.finishTool({
      role: 'toolResult',
      toolCallId: 'c',
      toolName: 'tool',
      content: [
        { type: 'text', text: this.#hostile },
        { type: 'image', data: 'AAAA'.repeat(2048), mimeType: 'image/png\u001b[31m' },
      ],
      isError: true,
      timestamp: 0,
    });
    await this.#agent.session.screen.waitFor('state: thinking');
    this.#agent.controls.finishModel(assistant([{ type: 'text', text: 'done' }]));
    return await this.#agent.session.screen.waitFor('state: ready');
  }
}
