// Adapted from Pi 1.5.2 coding-agent/src/extensions/codemode/{tool,execute}.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import type { CodemodeOutputItem, CodemodeResult } from '@earendil-works/pi-codemode';
import type { ImageContent, TextContent } from '@earendil-works/pi-ai';
import type { DefinitionResult } from './definition.ts';
import { errorText } from './definition.ts';
import type { CodemodeExecution } from './codemode-execution.ts';
import { CodemodeImages } from './codemode-images.ts';
import { writeOutputFile } from './output-files.ts';
import { sliceHead, sliceTail } from './text-slice.ts';

const outputItems = (result: CodemodeResult): CodemodeOutputItem[] => {
  const output = [...result.output];
  if (result.ok && result.value !== undefined) {
    output.push({
      type: 'text',
      text:
        typeof result.value === 'string'
          ? result.value
          : (JSON.stringify(result.value) ?? 'undefined'),
    });
  }
  return output;
};

const consoleOutput = (output: readonly CodemodeOutputItem[]): TextContent[] => {
  const lines = output.flatMap((item) => (item.type === 'text' && item.console ? [item.text] : []));
  return lines.length > 0
    ? [{ type: 'text', text: `<console_output>\n${lines.join('\n')}\n</console_output>` }]
    : [];
};

// Keep separate text emissions and console output, rather than ambiguously concatenating them.
const formatOutput = (output: readonly CodemodeOutputItem[]): (TextContent | ImageContent)[] => {
  const total = output.filter((item) => item.type === 'text' && !item.console).length;
  const items: (TextContent | ImageContent)[] = [];
  let index = 0;
  for (const item of output) {
    if (item.type === 'image') {
      items.push(item);
    } else if (!item.console) {
      index++;
      items.push({
        type: 'text',
        text: total > 1 ? `==> text ${index}/${total} <==\n${item.text}` : item.text,
      });
    }
  }
  return [...items, ...consoleOutput(output)];
};

/** Owns one result's ordered content, truncation artifact and image persistence. */
export class CodemodeOutput {
  readonly #directory: string;
  #content: (TextContent | ImageContent)[];
  #fullOutputPath: string | undefined;

  public constructor(result: CodemodeResult, directory: string) {
    this.#directory = directory;
    this.#content = formatOutput(outputItems(result));
  }

  public describe(result: CodemodeResult, execution: CodemodeExecution): void {
    if (execution.hasUnfinishedCalls()) {
      this.#content.push({
        type: 'text',
        text: 'Nested tool effects remain unfinished/uncertain. Cancellation was requested; abort-ignoring tools may still complete after this execution.',
      });
    }
    if (!result.ok) {
      this.#content.push({
        type: 'text',
        text: `Script error (${result.error.kind}): ${result.error.stack ?? result.error.message}\nTool calls already made are not undone: ${
          execution
            .snapshot()
            .calls.map((call) => `${call.name} (${call.status})`)
            .join(', ') || 'none'
        }`,
      });
    }
  }

  public async result(ok: boolean, maxChars: number): Promise<DefinitionResult> {
    const text = this.#content
      .filter((item) => item.type === 'text')
      .map((item) => item.text)
      .join('\n');
    if (text.length > maxChars) {
      await this.#truncate(text, maxChars);
    }
    this.#content = await new CodemodeImages(this.#directory).save(this.#content);
    return {
      content: [
        { type: 'text', text: ok ? 'Script completed' : 'Script failed' },
        ...this.#content,
      ],
      ...(this.#fullOutputPath ? { details: { fullOutputPath: this.#fullOutputPath } } : {}),
      isError: !ok,
    };
  }

  async #truncate(text: string, maxChars: number): Promise<void> {
    try {
      this.#fullOutputPath = await writeOutputFile('zf-codemode', '.txt', text, this.#directory);
    } catch (error) {
      this.#fullOutputPath = undefined;
      this.#content.push({ type: 'text', text: `Could not save full output: ${errorText(error)}` });
    }
    const half = Math.floor(maxChars / 2);
    this.#content = [
      {
        type: 'text',
        text: `${sliceHead(text, half)}\n…output truncated…\n${sliceTail(text, half)}\n${this.#fullOutputPath ? `Full output: ${this.#fullOutputPath}` : 'Full output could not be saved'}`,
      },
      ...this.#content.filter((item) => item.type === 'image'),
    ];
  }
}
