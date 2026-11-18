import type {
  AgentToolResult,
  FindToolDetails,
  FindToolInput,
  GrepToolDetails,
  GrepToolInput,
} from '@earendil-works/pi-coding-agent';

export type SearchResult = AgentToolResult<FindToolDetails | GrepToolDetails | undefined>;
export interface SearchCall {
  readonly toolCallId: string;
  readonly args: FindToolInput | GrepToolInput;
  readonly signal: AbortSignal | undefined;
  readonly onUpdate: ((result: SearchResult) => void) | undefined;
}
export interface SearchJob {
  readonly name: 'grep' | 'find';
  readonly cwd: string;
  readonly call: Omit<SearchCall, 'onUpdate'>;
  readonly remote: boolean;
}
export type WorkerMessage =
  | { readonly type: 'directory'; readonly id: number; readonly path: string }
  | { readonly type: 'result'; readonly result: SearchResult }
  | { readonly type: 'update'; readonly result: SearchResult }
  | { readonly type: 'error'; readonly error: string }
  | { readonly type: 'directory-aborted' };
export type DirectoryReply =
  | { readonly id: number; readonly value: boolean; readonly aborted: boolean }
  | { readonly id: number; readonly error: string };
