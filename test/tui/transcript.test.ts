import { expect, test } from 'vite-plus/test';
import { StateView, TranscriptView } from '../../packages/tui/src/index.ts';

test('transcript and state rendering are bounded regardless of run size', async () => {
  const messages = Array.from({ length: 5000 }, (_, index) => ({
    role: index % 2 === 0 ? 'user' : 'assistant',
    content:
      index % 2 === 0
        ? `line\n`.repeat(500) + index
        : [{ type: 'text', text: `${'word '.repeat(5000)}${index}` }],
  }));
  const transcript = new TranscriptView({ maxMessages: 50, maxLines: 120 });
  transcript.setMessages(messages);
  const rows = transcript.render(60);
  expect(rows.length).toBeLessThanOrEqual(120);
  expect(rows.join('\n')).toContain('earlier');
  expect(rows.join('\n')).toContain('4999');
  expect(rows.join('\n')).not.toContain(' 100 ');

  const state = new StateView({ maxLines: 30 });
  state.setView({
    value: 'ready',
    context: Object.fromEntries(Array.from({ length: 20_000 }, (_, i) => [`k${i}`, i])),
  });
  const stateRows = state.render(40);
  expect(stateRows.length).toBeLessThanOrEqual(30 + 3);
  expect(stateRows.join('\n')).toContain('more lines');
});

test('the transcript wraps settled history once per width and only as far as the row bound needs', () => {
  const reads = { visited: 0 };
  const message = (text: string): object =>
    Object.defineProperty({ role: 'user' }, 'content', {
      enumerable: true,
      get: () => {
        reads.visited += 1;
        return text;
      },
    });
  const transcript = new TranscriptView({ maxMessages: 100_000, maxLines: 50 });
  transcript.setMessages(Array.from({ length: 20_000 }, (_, index) => message(`message ${index}`)));
  const first = transcript.render(60);
  expect(first.length).toBeLessThanOrEqual(50);
  expect(first.join('\n')).toContain('message 19999');
  expect(first.join('\n')).toContain('earlier output hidden');
  // Newest-first wrapping stops once the bound is filled.
  expect(reads.visited).toBeLessThanOrEqual(60);
  const settledReads = reads.visited;
  for (let token = 0; token < 20; token += 1) {
    transcript.setStreaming({
      role: 'assistant',
      content: [{ type: 'text', text: `token ${token}` }],
    });
    expect(transcript.render(60).join('\n')).toContain(`token ${token}`);
  }
  expect(reads.visited).toBe(settledReads);
  transcript.render(30);
  expect(reads.visited).toBeGreaterThan(settledReads);
  // Fewer rows than the bound: no marker, nothing hidden.
  const small = new TranscriptView({ maxLines: 50 });
  small.setMessages([message('only')]);
  expect(small.render(40).join('\n')).not.toContain('hidden');
});
