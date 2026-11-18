import type {
  AssistantMessage,
  AssistantMessageEvent,
  Message,
  ToolCall,
  ToolResultMessage,
} from '@earendil-works/pi-ai';
import { gate } from './gate.ts';

/** Test control over the fixture agent's model and tool calls. */
export class AgentControls {
  public observeModel: (event: AssistantMessageEvent) => void = () => {};
  public observeTool: (result: ToolResultMessage) => void = () => {};
  public requests: Message[][] = [];
  public aborted = 0;
  #model = gate<AssistantMessage>();
  #tool = gate<ToolResultMessage>();

  // Keep the original bound callback surface while prototype methods own gate behavior.
  public readonly respond = (messages: Message[], signal: AbortSignal): Promise<AssistantMessage> =>
    this.#respond(messages, signal);

  public readonly run = (call: ToolCall, signal: AbortSignal): Promise<ToolResultMessage> =>
    this.#run(call, signal);

  #respond(messages: Message[], signal: AbortSignal): Promise<AssistantMessage> {
    this.requests.push(messages);
    this.#model = gate<AssistantMessage>();
    const pending = this.#model;
    signal.addEventListener('abort', () => (this.aborted += 1), { once: true });
    return new Promise((resolvePromise, reject) => {
      signal.addEventListener('abort', () => reject(signal.reason), { once: true });
      void pending.promise.then(resolvePromise);
    });
  }

  #run(_call: ToolCall, signal: AbortSignal): Promise<ToolResultMessage> {
    this.#tool = gate<ToolResultMessage>();
    const pending = this.#tool;
    return new Promise((resolvePromise, reject) => {
      signal.addEventListener('abort', () => reject(signal.reason), { once: true });
      void pending.promise.then(resolvePromise);
    });
  }

  public finishModel(message: AssistantMessage): void {
    this.#model.resolve(message);
  }

  public finishTool(result: ToolResultMessage): void {
    this.#tool.resolve(result);
  }
}
