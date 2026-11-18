import type { ToolDefinition as PiToolDefinition } from '@earendil-works/pi-coding-agent';
import type { ExecutableTool } from '@zf/core';
import type { TSchema } from 'typebox';
import { wrapToolDefinition } from './definition.ts';

type PiContext = Parameters<PiToolDefinition['execute']>[4];

/** `precondition` runs before each Pi execution; a thrown error becomes the tool error result. */
export const fromPi = <S extends TSchema, D>(
  definition: PiToolDefinition<S, D>,
  precondition?: () => void,
  context?: Pick<PiContext, 'model'>,
): ExecutableTool =>
  wrapToolDefinition<S, D>({
    name: definition.name,
    label: definition.label,
    description: definition.description,
    parameters: definition.parameters,
    promptSnippet: definition.promptSnippet,
    promptGuidelines: definition.promptGuidelines,
    ...(definition.outputSchema === undefined ? {} : { outputSchema: definition.outputSchema }),
    ...(definition.constrainedSampling === undefined
      ? {}
      : { constrainedSampling: definition.constrainedSampling }),
    ...(definition.prepareArguments === undefined
      ? {}
      : { prepareArguments: definition.prepareArguments }),
    execute: async ({ toolCallId, args, signal, onUpdate }) => {
      precondition?.();
      // Foreign ABI boundary for pinned Pi 1.5.2 built-ins: only read supplies a
      // typed model subset. Its native execute uses ctx?.cwd/model, but the public
      // type requires ExtensionToolContext; omitted cwd retains the factory cwd.
      // The public root read wrapper cannot inject context. No session is fabricated.
      // Other tools receive undefined, keeping bash/PowerShell's sessionManager and
      // thinkingLevel environment-injection branch inactive. Re-audit on Pi upgrades.
      return definition.execute(toolCallId, args, signal, onUpdate, context as PiContext);
    },
  });
