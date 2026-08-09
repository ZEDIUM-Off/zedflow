// Executable oracle over the frozen Pi compat implementation.
// A tiny loader supplies inert stand-ins for optional packages that are not
// needed by compat's faux transport but are re-exported by its entrypoint.

import { register } from "node:module";

register(
	`data:text/javascript,${encodeURIComponent(`
export async function resolve(specifier, context, nextResolve) {
	if (specifier === "typebox") return { url: "oracle:typebox", shortCircuit: true };
	if (specifier === "typebox/compile") return { url: "oracle:typebox-compile", shortCircuit: true };
	if (specifier === "typebox/value") return { url: "oracle:typebox-value", shortCircuit: true };
	if (specifier === "partial-json") return { url: "oracle:partial-json", shortCircuit: true };
	return nextResolve(specifier, context);
}
export async function load(url, context, nextLoad) {
	if (url === "oracle:typebox") return { format: "module", source: "const noop = new Proxy(() => ({}), { get: () => noop }); export const Type = noop;", shortCircuit: true };
	if (url === "oracle:typebox-compile") return { format: "module", source: "export const Compile = () => ({ Check: () => true, Errors: () => [] });", shortCircuit: true };
	if (url === "oracle:typebox-value") return { format: "module", source: "export const Value = {};", shortCircuit: true };
	if (url === "oracle:partial-json") return { format: "module", source: "export const parse = JSON.parse;", shortCircuit: true };
	return nextLoad(url, context);
}
`)}`,
	import.meta.url,
);

const compat = await import("../../../../references/pi/packages/ai/src/compat.ts");
const { complete, fauxAssistantMessage, registerFauxProvider, resetApiProviders } = compat;

const apis = [
	"anthropic-messages",
	"openai-completions",
	"openai-responses",
	"openai-codex-responses",
	"azure-openai-responses",
	"google-generative-ai",
	"google-vertex",
	"mistral-conversations",
	"bedrock-converse-stream",
];

function describeContext(context) {
	return context.messages
		.map((message) => {
			const content = message.content;
			return `${message.role}:${typeof content === "string" ? `text:${content.length}` : `blocks:${content.length}`}`;
		})
		.join("|");
}

const emptyContexts = [
	["content-array", { messages: [{ role: "user", content: [] }] }],
	["empty-string", { messages: [{ role: "user", content: "" }] }],
	["whitespace", { messages: [{ role: "user", content: "   \n\t  " }] }],
	[
		"empty-assistant",
		{ messages: [{ role: "user", content: "hello" }, { role: "assistant", content: [] }, { role: "user", content: "respond" }] },
	],
];
const emptyRegistration = registerFauxProvider({ api: "oracle-empty" });
emptyRegistration.setResponses(
	emptyContexts.map(() => (context) => fauxAssistantMessage(describeContext(context), { timestamp: 1 })),
);
const empty = [];
for (const [input, context] of emptyContexts) {
	const response = await complete(emptyRegistration.getModel(), context);
	empty.push({
		input,
		role: response.role,
		contentDefined: response.content !== undefined,
		error: response.errorMessage !== undefined,
		text: response.content[0]?.text,
	});
}
emptyRegistration.unregister();

const dispatch = [];
for (const api of apis) {
	const registration = registerFauxProvider({ api, provider: `oracle-${api}`, models: [{ id: "oracle-model" }] });
	registration.setResponses([
		(context, _options, _state, model) =>
			fauxAssistantMessage(`${model.api}:${describeContext(context)}`, { timestamp: 1 }),
	]);
	const response = await complete(registration.getModel(), { messages: [{ role: "user", content: api }] });
	dispatch.push({
		api: response.api,
		provider: response.provider,
		model: response.model,
		role: response.role,
		text: response.content[0]?.text,
	});
	registration.unregister();
}
resetApiProviders();

process.stdout.write(JSON.stringify({ empty, dispatch }));
