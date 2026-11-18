import { bold, cyan, dim, plain, red, yellow } from './style.ts';
import { sanitizeInline, sanitizeLines } from './text.ts';
import {
  ASSISTANT,
  CALL,
  FAILURE,
  RESULT,
  THINKING,
  USER,
  blocks,
  clip,
  imageEntry,
  isRecord,
  text,
} from './transcript-content.ts';
import type { Entry } from './transcript-content.ts';

const TEXT_LINES = 80;
const THINKING_LINES = 6;
const RESULT_LINES = 20;
const ARGUMENT_CHARACTERS = 160;

const argumentsText = (value: unknown): string => {
  try {
    return JSON.stringify(value) ?? '';
  } catch {
    return '[unserializable arguments]';
  }
};

const callEntry = (block: Record<string, unknown>): Entry => ({
  gutter: CALL,
  style: cyan,
  lines: [
    sanitizeInline(
      `${text(block['name'])}(${sanitizeInline(argumentsText(block['arguments']), ARGUMENT_CHARACTERS)})`,
      ARGUMENT_CHARACTERS + 60,
    ),
  ],
});

const userEntries = (content: unknown): Entry[] => {
  if (typeof content === 'string') {
    return [{ gutter: USER, style: bold, lines: clip(sanitizeLines(content), TEXT_LINES) }];
  }
  return blocks(content).flatMap((block): Entry[] => {
    if (!isRecord(block)) {
      return [];
    }
    if (block['type'] === 'image') {
      return [imageEntry(block)];
    }
    return block['type'] === 'text'
      ? [{ gutter: USER, style: bold, lines: clip(sanitizeLines(text(block['text'])), TEXT_LINES) }]
      : [];
  });
};

const assistantBlockEntries = (block: unknown): Entry[] => {
  if (!isRecord(block)) {
    return [];
  }
  if (block['type'] === 'text') {
    return [
      {
        gutter: ASSISTANT,
        style: plain,
        lines: clip(sanitizeLines(text(block['text'])), TEXT_LINES),
      },
    ];
  }
  if (block['type'] === 'thinking') {
    return [
      {
        gutter: THINKING,
        style: dim,
        lines: clip(sanitizeLines(text(block['thinking'])), THINKING_LINES),
      },
    ];
  }
  return block['type'] === 'toolCall' ? [callEntry(block)] : [];
};

const assistantEntries = (message: Record<string, unknown>): Entry[] => {
  const entries = blocks(message['content']).flatMap(assistantBlockEntries);
  const failure = text(message['errorMessage']);
  return failure === ''
    ? entries
    : [
        ...entries,
        { gutter: FAILURE, style: red, lines: clip(sanitizeLines(failure), RESULT_LINES) },
      ];
};

export const resultEntries = (message: Record<string, unknown>): Entry[] => {
  const failed = message['isError'] === true;
  const style = failed ? red : plain;
  const name = sanitizeInline(text(message['toolName']), 60);
  const body = blocks(message['content']).flatMap((block): Entry[] => {
    if (!isRecord(block)) {
      return [];
    }
    if (block['type'] === 'image') {
      return [imageEntry(block)];
    }
    return block['type'] === 'text'
      ? [{ gutter: RESULT, style, lines: clip(sanitizeLines(text(block['text'])), RESULT_LINES) }]
      : [];
  });
  return [
    { gutter: RESULT, style: failed ? red : yellow, lines: [`${name}${failed ? ' failed' : ''}`] },
    ...body,
  ];
};

export const messageEntries = (message: unknown): Entry[] => {
  if (!isRecord(message)) {
    return [];
  }
  switch (message['role']) {
    case 'user':
      return userEntries(message['content']);
    case 'assistant':
      return assistantEntries(message);
    case 'toolResult':
      return resultEntries(message);
    default:
      return [];
  }
};
