#!/usr/bin/env node
import assert from "node:assert/strict";
import process from "node:process";
import xterm from "@xterm/headless";
import { Text, TUI } from "./packages/tui/src/index.ts";
import { AssistantMessageComponent } from "./packages/coding-agent/src/modes/interactive/components/assistant-message.ts";
import { CustomEditor } from "./packages/coding-agent/src/modes/interactive/components/custom-editor.ts";
import { FooterComponent } from "./packages/coding-agent/src/modes/interactive/components/footer.ts";
import { SettingsSelectorComponent } from "./packages/coding-agent/src/modes/interactive/components/settings-selector.ts";
import { IdleStatus } from "./packages/coding-agent/src/modes/interactive/components/status-indicator.ts";
import { ToolExecutionComponent } from "./packages/coding-agent/src/modes/interactive/components/tool-execution.ts";
import { InteractiveMode } from "./packages/coding-agent/src/modes/interactive/interactive-mode.ts";
import { Theme, initTheme, setRegisteredThemes } from "./packages/coding-agent/src/modes/interactive/theme/theme.ts";

const SELF_CHECK_FIXTURE = {
  version: 2,
  dimensions: { columns: 8, rows: 2 },
  capabilities: { colors: 256, unicode: true, kittyKeyboard: true },
  events: [{ type: "lifecycle", name: "message_update", data: { content: "ok", timestamp: 1 } }],
};
const plain = (text) => text;
const plainTheme = new Theme(
  Object.fromEntries([
    "accent", "border", "borderAccent", "borderMuted", "success", "error", "warning",
    "muted", "dim", "text", "thinkingText", "userMessageText", "customMessageText",
    "customMessageLabel", "toolTitle", "toolOutput", "mdHeading", "mdLink", "mdLinkUrl",
    "mdCode", "mdCodeBlock", "mdCodeBlockBorder", "mdQuote", "mdQuoteBorder", "mdHr",
    "mdListBullet", "toolDiffAdded", "toolDiffRemoved", "toolDiffContext", "thinkingOff",
    "thinkingMinimal", "thinkingLow", "thinkingMedium", "thinkingHigh", "thinkingXhigh",
    "bashMode",
  ].map(key => [key, ""])),
  { toolPendingBg: "", toolSuccessBg: "", toolErrorBg: "" },
  "256color",
  { name: "oracle-plain" },
);
plainTheme.bold = plain;
plainTheme.italic = plain;
plainTheme.underline = plain;
plainTheme.inverse = plain;
plainTheme.strikethrough = plain;
setRegisteredThemes([plainTheme]);
const editorTheme = {
  borderColor: plain,
  selectList: { selectedPrefix: plain, selectedText: plain, description: plain, scrollInfo: plain, noMatch: plain },
};
const markdownTheme = {
  heading: plain, link: plain, linkUrl: plain, code: plain, codeBlock: plain, codeBlockBorder: plain,
  quote: plain, quoteBorder: plain, hr: plain, listBullet: plain, bold: plain, italic: plain,
  strikethrough: plain, underline: plain,
};
const settingsConfig = {
  autoCompact: true, showImages: false, imageWidthCells: 80, autoResizeImages: true,
  blockImages: false, enableSkillCommands: true, steeringMode: "one-at-a-time",
  followUpMode: "one-at-a-time", transport: "auto", httpIdleTimeoutMs: 300000,
  thinkingLevel: "off", availableThinkingLevels: ["off"], currentTheme: "oracle-plain",
  terminalTheme: "dark", availableThemes: ["oracle-plain"], hideThinkingBlock: false,
  collapseChangelog: false, enableInstallTelemetry: false, doubleEscapeAction: "none",
  treeFilterMode: "default", showHardwareCursor: false, editorPaddingX: 0, outputPad: 1,
  autocompleteMaxVisible: 5, quietStartup: true, defaultProjectTrust: "ask",
  clearOnShrink: false, showTerminalProgress: false, warnings: {},
};
const noop = () => {};

function validateFixture(value) {
  assert.equal(value?.version, 2, "fixture.version must be 2");
  for (const key of ["columns", "rows"])
    assert(Number.isInteger(value?.dimensions?.[key]) && value.dimensions[key] > 0, `dimensions.${key} must be a positive integer`);
  assert(value?.capabilities && Array.isArray(value?.events), "fixture requires capabilities and events");
  for (const event of value.events) assert(["input", "resize", "lifecycle"].includes(event?.type), `unknown event type: ${event?.type}`);
}

function cleanMetadata(value) {
  if (Array.isArray(value)) return value.map(cleanMetadata);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value)
    .filter(([key]) => !["timestamp", "cwd", "path", "query"].includes(key))
    .map(([key, child]) => [key, cleanMetadata(child)]));
}

function piWcwidth(codepoint) {
  if (codepoint < 32 || (codepoint >= 0x7f && codepoint < 0xa0)) return 0;
  if ((codepoint >= 0x300 && codepoint <= 0x36f) || (codepoint >= 0x1ab0 && codepoint <= 0x1aff)
    || (codepoint >= 0x1dc0 && codepoint <= 0x1dff) || (codepoint >= 0x20d0 && codepoint <= 0x20ff)
    || (codepoint >= 0xfe00 && codepoint <= 0xfe0f)) return 0;
  if ((codepoint >= 0x1100 && codepoint <= 0x115f) || (codepoint >= 0x2e80 && codepoint <= 0xa4cf)
    || (codepoint >= 0xac00 && codepoint <= 0xd7a3) || (codepoint >= 0xf900 && codepoint <= 0xfaff)
    || (codepoint >= 0x1f300 && codepoint <= 0x1faff) || (codepoint >= 0x20000 && codepoint <= 0x3fffd)) return 2;
  return 1;
}

function styleOf(cell) {
  const style = {};
  const fgMode = cell.getFgColorMode();
  const bgMode = cell.getBgColorMode();
  if (fgMode) style.fg = `${fgMode}:${cell.getFgColor()}`;
  if (bgMode) style.bg = `${bgMode}:${cell.getBgColor()}`;
  for (const [name, method] of [["bold", "isBold"], ["dim", "isDim"], ["italic", "isItalic"], ["underline", "isUnderline"], ["inverse", "isInverse"], ["invisible", "isInvisible"], ["strikethrough", "isStrikethrough"]]) if (cell[method]()) style[name] = true;
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
    while (row.length && (row.at(-1).text === "" || row.at(-1).text === " ") && row.at(-1).width === 1 && !row.at(-1).style) row.pop();
    cells.push(row);
  }
  return { cells, cursor: { x: buffer.cursorX, y: buffer.cursorY, visible: cursorVisible } };
}

async function write(terminal, data) { return new Promise(resolve => terminal.write(data, resolve)); }

function makeScreen(columns, rows) {
  const terminal = new xterm.Terminal({ cols: columns, rows, allowProposedApi: true, disableStdin: true });
  terminal.unicode.register({ version: "pi", wcwidth: piWcwidth, charProperties: codepoint => (piWcwidth(codepoint) << 1) | (piWcwidth(codepoint) === 0 ? 1 : 0) });
  terminal.unicode.activeVersion = "pi";
  return terminal;
}

async function render(fixture) {
  validateFixture(fixture);
  initTheme("oracle-plain");
  let columns = fixture.dimensions.columns;
  let rows = fixture.dimensions.rows;
  let screen = makeScreen(columns, rows);
  const terminalPort = {
    get columns() { return columns; }, get rows() { return rows; }, kittyProtocolActive: true,
    start() {}, stop() {}, async drainInput() {}, write() {}, moveBy() {}, hideCursor() {}, showCursor() {},
    clearLine() {}, clearFromCursor() {}, clearScreen() {}, setTitle() {}, setProgress() {},
  };
  const tui = new TUI(terminalPort); // The component TUI owns composition; xterm owns terminal cells.
  const editor = new CustomEditor(tui, editorTheme, { matches: () => false });
  const assistant = new AssistantMessageComponent(undefined, false, markdownTheme);
  const idle = new IdleStatus();
  const footer = new FooterComponent(
    {
      state: { model: undefined, thinkingLevel: "off" },
      sessionManager: {
        getEntries: () => [],
        getCwd: () => process.env.ZEDFLOW_ORACLE_CWD ?? process.cwd(),
        getSessionName: () => undefined,
      },
      getContextUsage: () => undefined,
      modelRegistry: { isUsingOAuth: () => false },
    },
    {
      getGitBranch: () => undefined,
      getAvailableProviderCount: () => 0,
      getExtensionStatuses: () => new Map(),
    },
  );
  let selector;
  let message = { role: "assistant", content: [], stopReason: "complete" };
  let showAssistant = false;
  let tool;
  let overlay = false;
  let compacting = false;
  // Invoke frozen InteractiveMode's own submit switch without constructing its
  // concrete ProcessTerminal/runtime graph. Command methods mount real frozen
  // components on this deterministic TUI seam.
  const dispatcher = Object.create(InteractiveMode.prototype);
  dispatcher.defaultEditor = editor;
  dispatcher.editor = editor;
  dispatcher.showSettingsSelector = () => {
    const callbacks = new Proxy(
      { onCancel: () => { overlay = false; tui.setFocus(editor); } },
      { get: (target, key) => target[key] ?? noop },
    );
    selector = new SettingsSelectorComponent(settingsConfig, callbacks);
    overlay = true;
    tui.setFocus(selector.settingsList);
  };
  dispatcher.handleCompactCommand = async () => { compacting = true; };
  dispatcher.showModelSelector = noop;
  dispatcher.handleModelCommand = noop;
  InteractiveMode.prototype.setupEditorSubmitHandler.call(dispatcher);
  const frames = [];
  const inputs = [];
  const lifecycle = [];

  const compose = () => {
    tui.clear();
    if (showAssistant) tui.addChild(assistant);
    if (tool) tui.addChild(tool);
    if (compacting) tui.addChild(new Text("Compacting...", 1, 0));
    tui.addChild(idle);
    tui.addChild(editor);
    tui.addChild(footer);
    if (overlay && selector) tui.addChild(selector);
    return tui.render(columns);
  };
  const redraw = async () => {
    const lines = compose();
    await write(screen, `\x1b[2J\x1b[H${lines.join("\r\n")}`);
  };

  for (const event of fixture.events) {
    if (event.type === "input") {
      inputs.push(event.data);
      if (overlay) {
        tui.focusedComponent?.handleInput?.(event.data);
      } else {
        editor.handleInput(event.data);
      }
    } else if (event.type === "resize") {
      columns = event.columns;
      rows = event.rows;
      screen.resize(columns, rows);
    } else {
      const data = cleanMetadata(event.data ?? null);
      lifecycle.push({ name: event.name, data });
      if (event.name === "message_start") {
        message = { role: "assistant", content: [], stopReason: "complete" };
        showAssistant = true;
      }
      if (event.name === "message_update" || event.name === "message_end") {
        const content = typeof data?.content === "string" ? data.content : "";
        message = { ...message, content: content ? [{ type: "text", text: content }] : [], stopReason: "complete" };
        assistant.updateContent(message);
        showAssistant = true;
      }
      if (event.name === "tool_start") {
        tool = new ToolExecutionComponent(data?.tool ?? "tool", "oracle", undefined, {}, undefined, tui, process.cwd());
        tool.markExecutionStarted();
      }
      if (event.name === "tool_update" || event.name === "tool_end") tool?.updateResult({ content: [{ type: "text", text: data?.content ?? "" }], isError: false });
      if (event.name === "compaction_start") compacting = true;
      if (event.name === "compaction_end") compacting = false;
      if (event.name === "session") overlay = true;
      if (event.name === "abort") {
        message = { ...message, stopReason: "aborted", errorMessage: "Operation aborted" };
        assistant.updateContent(message);
        showAssistant = true;
      }
      if (event.name === "error") {
        message = { ...message, stopReason: "error", errorMessage: data?.message ?? "Unknown error" };
        assistant.updateContent(message);
        showAssistant = true;
      }
    }
    await redraw();
    frames.push(frame(screen, !overlay));
  }
  screen.dispose();
  return { version: 2, frames, inputs, lifecycle };
}

if (process.argv.includes("--self-check")) {
  validateFixture(SELF_CHECK_FIXTURE);
  assert.deepEqual(cleanMetadata(SELF_CHECK_FIXTURE.events[0].data), { content: "ok" });
  process.stdout.write(`${JSON.stringify({ version: 2, protocol: "component-oracle" })}\n`);
} else {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  process.stdout.write(`${JSON.stringify(await render(JSON.parse(Buffer.concat(chunks).toString("utf8"))))}\n`);
}
