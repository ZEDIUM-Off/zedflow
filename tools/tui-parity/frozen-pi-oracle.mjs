#!/usr/bin/env node
import assert from "node:assert/strict";
import process from "node:process";

const SELF_CHECK_FIXTURE = {
  version: 1,
  dimensions: { columns: 8, rows: 2 },
  capabilities: { colors: 256, unicode: true, kittyKeyboard: true },
  events: [
    { type: "write", data: "\u001b[31mPi\u001b[0m" },
    { type: "input", data: "x" },
    { type: "lifecycle", name: "message_end", data: { timestamp: "ignored" } },
  ],
};

function validateFixture(value) {
  assert.equal(value?.version, 1, "fixture.version must be 1");
  for (const key of ["columns", "rows"])
    assert(Number.isInteger(value?.dimensions?.[key]) && value.dimensions[key] > 0, `dimensions.${key} must be a positive integer`);
  assert(value?.capabilities && Array.isArray(value?.events), "fixture requires capabilities and events");
  for (const event of value.events) assert(["write", "input", "resize", "lifecycle"].includes(event?.type), `unknown event type: ${event?.type}`);
}

function cleanMetadata(value) {
  if (Array.isArray(value)) return value.map(cleanMetadata);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value)
    .filter(([key]) => !["timestamp", "cwd", "path", "query"].includes(key))
    .map(([key, child]) => [key, cleanMetadata(child)]));
}

function write(terminal, data) {
  return new Promise((resolve) => terminal.write(data, resolve));
}

// Match frozen Pi's utils.ts terminal-width policy rather than xterm's legacy
// default, which treats otherwise full-width emoji as one cell.
function piWcwidth(codepoint) {
  if (codepoint < 32 || (codepoint >= 0x7f && codepoint < 0xa0)) return 0;
  if ((codepoint >= 0x300 && codepoint <= 0x36f)
    || (codepoint >= 0x1ab0 && codepoint <= 0x1aff)
    || (codepoint >= 0x1dc0 && codepoint <= 0x1dff)
    || (codepoint >= 0x20d0 && codepoint <= 0x20ff)
    || (codepoint >= 0xfe00 && codepoint <= 0xfe0f)) return 0;
  if ((codepoint >= 0x1100 && codepoint <= 0x115f)
    || (codepoint >= 0x2e80 && codepoint <= 0xa4cf)
    || (codepoint >= 0xac00 && codepoint <= 0xd7a3)
    || (codepoint >= 0xf900 && codepoint <= 0xfaff)
    || (codepoint >= 0x1f300 && codepoint <= 0x1faff)
    || (codepoint >= 0x20000 && codepoint <= 0x3fffd)) return 2;
  return 1;
}

function styleOf(cell) {
  const style = {};
  const fgMode = cell.getFgColorMode();
  const bgMode = cell.getBgColorMode();
  if (fgMode) style.fg = `${fgMode}:${cell.getFgColor()}`;
  if (bgMode) style.bg = `${bgMode}:${cell.getBgColor()}`;
  for (const [name, method] of [
    ["bold", "isBold"], ["dim", "isDim"], ["italic", "isItalic"],
    ["underline", "isUnderline"], ["inverse", "isInverse"],
    ["invisible", "isInvisible"], ["strikethrough", "isStrikethrough"],
  ]) if (cell[method]()) style[name] = true;
  return style;
}

function frame(terminal, cursorVisible) {
  const buffer = terminal.buffer.active;
  const cells = [];
  for (let y = 0; y < terminal.rows; y++) {
    const source = buffer.getLine(buffer.viewportY + y);
    const row = [];
    for (let x = 0; source && x < terminal.cols; x++) {
      const cell = source.getCell(x);
      const style = styleOf(cell);
      const normalized = { text: cell.getChars(), width: cell.getWidth() };
      if (Object.keys(style).length) normalized.style = style;
      row.push(normalized);
    }
    while (row.length && row.at(-1).text === "" && row.at(-1).width === 1 && !row.at(-1).style) row.pop();
    cells.push(row);
  }
  return {
    cells,
    cursor: { x: buffer.cursorX, y: buffer.cursorY, visible: cursorVisible },
  };
}

async function render(fixture) {
  validateFixture(fixture);
  let xterm;
  try {
    xterm = await import("@xterm/headless");
  } catch (error) {
    throw new Error("missing frozen Pi dependencies; run tools/tui-parity/run.py --prepare first", { cause: error });
  }
  const Terminal = xterm.Terminal ?? xterm.default?.Terminal;
  const terminal = new Terminal({ cols: fixture.dimensions.columns, rows: fixture.dimensions.rows, allowProposedApi: true, disableStdin: true });
  terminal.unicode.register({
    version: "pi",
    wcwidth: piWcwidth,
    // xterm encodes width in bits 1-2 and the grapheme-join flag in bit 0.
    charProperties(codepoint) {
      const width = piWcwidth(codepoint);
      return (width << 1) | (width === 0 ? 1 : 0);
    },
  });
  terminal.unicode.activeVersion = "pi";
  const frames = [];
  const inputs = [];
  const lifecycle = [];
  let cursorVisible = true;
  for (const event of fixture.events) {
    if (event.type === "write") {
      await write(terminal, event.data);
      if (event.data.includes("\u001b[?25l")) cursorVisible = false;
      if (event.data.includes("\u001b[?25h")) cursorVisible = true;
    }
    else if (event.type === "input") inputs.push(event.data);
    else if (event.type === "resize") terminal.resize(event.columns, event.rows);
    else {
      lifecycle.push({ name: event.name, data: cleanMetadata(event.data ?? null) });
      if (event.render) await write(terminal, event.render);
    }
    frames.push(frame(terminal, cursorVisible));
  }
  terminal.dispose();
  return { version: 1, frames, inputs, lifecycle };
}

if (process.argv.includes("--self-check")) {
  validateFixture(SELF_CHECK_FIXTURE);
  assert.deepEqual(cleanMetadata(SELF_CHECK_FIXTURE.events[2].data), {});
  process.stdout.write(`${JSON.stringify({ version: 1, protocol: "ok" })}\n`);
} else {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  process.stdout.write(`${JSON.stringify(await render(JSON.parse(Buffer.concat(chunks).toString("utf8"))))}\n`);
}
