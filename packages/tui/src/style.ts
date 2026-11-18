import type { EditorTheme, SelectListTheme } from '@earendil-works/pi-tui';
import type { Style } from './text.ts';

const sgr =
  (open: number, close: number): Style =>
  (text) =>
    `\u001b[${open}m${text}\u001b[${close}m`;

export const bold = sgr(1, 22);
export const dim = sgr(2, 22);
export const red = sgr(31, 39);
export const green = sgr(32, 39);
export const yellow = sgr(33, 39);
export const cyan = sgr(36, 39);
export const plain: Style = (text) => text;

export const selectListTheme: SelectListTheme = {
  selectedPrefix: cyan,
  selectedText: bold,
  description: dim,
  scrollInfo: dim,
  noMatch: dim,
};

export const editorTheme: EditorTheme = { borderColor: dim, selectList: selectListTheme };
