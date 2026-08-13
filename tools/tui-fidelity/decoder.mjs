#!/usr/bin/env node
// The one decoder used for both real CLIs.  Input is a raw PTY capture.
import fs from "node:fs";
import xterm from "@xterm/headless";

const [columns, rows, source] = process.argv.slice(2);
const terminal = new xterm.Terminal({ cols: Number(columns), rows: Number(rows), allowProposedApi: true, disableStdin: true });
const data = fs.readFileSync(source);
await new Promise(resolve => terminal.write(data.toString("utf8"), resolve));
const buffer = terminal.buffer.active;
const cellStyle = cell => {
  const style = {};
  const fg = cell.getFgColorMode(); const bg = cell.getBgColorMode();
  if (fg) style.fg = `${fg}:${cell.getFgColor()}`;
  if (bg) style.bg = `${bg}:${cell.getBgColor()}`;
  for (const [key, method] of [["bold", "isBold"], ["dim", "isDim"], ["italic", "isItalic"], ["underline", "isUnderline"], ["inverse", "isInverse"], ["strikethrough", "isStrikethrough"]]) if (cell[method]()) style[key] = true;
  return style;
};
const cells = [];
for (let y = 0; y < terminal.rows; y++) {
  const row = []; const line = buffer.getLine(buffer.viewportY + y);
  for (let x = 0; line && x < terminal.cols; x++) {
    const cell = line.getCell(x); const style = cellStyle(cell);
    const value = { text: cell.getChars(), width: cell.getWidth() };
    if (Object.keys(style).length) value.style = style;
    row.push(value);
  }
  while (row.length && row.at(-1).text === "" && row.at(-1).width === 1 && !row.at(-1).style) row.pop();
  cells.push(row);
}
console.log(JSON.stringify({ cells, cursor: { x: buffer.cursorX, y: buffer.cursorY, visible: true } }));
