import { hyperlink, visibleWidth } from '@earendil-works/pi-tui';
import type { AuthEvent } from '@earendil-works/pi-ai';
import { dim, plain, yellow } from './style.ts';
import { sanitizeInline, sanitizeLines } from './text.ts';
import type { Style } from './text.ts';
import { wrapGutter } from './text-wrap.ts';

interface Notice {
  readonly lines: readonly string[];
  readonly link?: string;
  readonly style: Style;
}

const MAX_NOTICES = 8;
const MAX_URL_CHARACTERS = 2000;
const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

// Only web URLs become links; the parsed href has no control characters.
const webHref = (url: string): string | undefined => {
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'https:' || parsed.protocol === 'http:' ? parsed.href : undefined;
  } catch {
    return undefined;
  }
};

// A URL has no spaces, so split it by columns instead of words.
const chunkColumns = (text: string, width: number): string[] => {
  const rows: string[] = [];
  let row = '';
  for (const { segment } of segmenter.segment(text)) {
    if (row !== '' && visibleWidth(row + segment) > width) {
      rows.push(row);
      row = '';
    }
    row += segment;
  }
  return row === '' ? rows : [...rows, row];
};

const infoNotice = (
  message: string,
  links: Extract<AuthEvent, { type: 'info' }>['links'],
): Notice => {
  return {
    lines: [
      ...sanitizeLines(message),
      ...(links ?? []).map(
        (link) =>
          `${link.label ? `${sanitizeInline(link.label, 60)}: ` : ''}${sanitizeInline(link.url, MAX_URL_CHARACTERS)}`,
      ),
    ],
    style: plain,
  };
};

/** Retains only the newest provider notices, including safe web-link rendering. */
export class AuthNotices {
  #notices: Notice[] = [];

  public show(event: AuthEvent): void {
    this.#notices = [...this.#notices, this.#notice(event)].slice(-MAX_NOTICES);
  }

  public clear(): void {
    this.#notices = [];
  }

  public render(width: number): string[] {
    return this.#notices.flatMap((notice) => this.#renderNotice(notice, width));
  }

  #notice(event: AuthEvent): Notice {
    switch (event.type) {
      case 'info':
        return infoNotice(event.message, event.links);
      case 'auth_url':
        return {
          lines: [
            'Open this URL to sign in:',
            sanitizeInline(event.url, MAX_URL_CHARACTERS),
            ...(event.instructions ? sanitizeLines(event.instructions) : []),
          ],
          ...this.#link(event.url),
          style: yellow,
        };
      case 'device_code':
        return {
          lines: [
            `Open ${sanitizeInline(event.verificationUri, MAX_URL_CHARACTERS)} and enter code: ${sanitizeInline(event.userCode, 64)}`,
            'Waiting for authentication...',
          ],
          style: yellow,
        };
      case 'progress':
        return { lines: sanitizeLines(event.message), style: dim };
    }
  }

  #link(url: string): { readonly link: string } | Record<never, never> {
    const link = webHref(url);
    return link === undefined ? {} : { link };
  }

  #renderNotice(notice: Notice, width: number): string[] {
    if (notice.link === undefined) {
      return wrapGutter(notice.lines, { first: ' ', rest: ' ' }, { width, style: notice.style });
    }
    const [heading, url, ...rest] = notice.lines;
    const link = notice.link;
    const urlRows = chunkColumns(url ?? '', Math.max(1, width - 1)).map(
      (chunk) => ` ${hyperlink(chunk, link)}`,
    );
    return [
      ...wrapGutter([heading ?? ''], { first: ' ', rest: ' ' }, { width, style: notice.style }),
      ...urlRows,
      ...wrapGutter(rest, { first: ' ', rest: ' ' }, { width, style: notice.style }),
    ];
  }
}
