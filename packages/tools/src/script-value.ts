// Adapted from Pi 1.5.2 coding-agent/src/extensions/codemode/execute.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import { scriptResults } from './script-results.ts';
import { sliceHead } from './text-slice.ts';
const MAX_ERROR_CHARS = 500;
export const resultError = (result: ToolResultMessage): string => {
  let text = '';
  for (const block of result.content) {
    if (block.type === 'text') {
      text += sliceHead(block.text, MAX_ERROR_CHARS - text.length);
    }
    if (text.length >= MAX_ERROR_CHARS) {
      break;
    }
  }
  return text || `Tool ${sliceHead(result.toolName, 200)} failed`;
};

const resultText = (result: ToolResultMessage, fallback = ''): string =>
  result.content
    .filter((item) => item.type === 'text')
    .map((item) => item.text)
    .join('\n') || fallback;

export const scriptValue = (result: ToolResultMessage): unknown => {
  const structured = scriptResults.get(result);
  if (structured !== undefined) {
    return structured;
  }
  const details = result.details;
  // Structured error results resolve too (e.g. bash exit_code); scripts must inspect their fields.
  if (
    details &&
    typeof details === 'object' &&
    !Array.isArray(details) &&
    'structuredContent' in details
  ) {
    return details['structuredContent'];
  }
  if (result.isError) {
    throw new Error(resultText(result, `Tool ${result.toolName} failed`));
  }
  if (result.content.some((item) => item.type === 'image')) {
    return { content: result.content, details: result.details };
  }
  return resultText(result);
};
