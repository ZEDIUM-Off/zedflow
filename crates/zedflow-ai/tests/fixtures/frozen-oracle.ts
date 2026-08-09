// Dependency-free executable extraction of compat.ts registry/complete semantics.
// The transport is local and faux, so Cargo can run the frozen oracle offline.

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
const registry = new Map();

function registerApiProvider(provider) {
	registry.set(provider.api, {
		...provider,
		stream(model, context, options) {
			if (model.api !== provider.api) throw new Error(`Mismatched api: ${model.api} expected ${provider.api}`);
			return provider.stream(model, context, options);
		},
	});
}

async function complete(model, context) {
	const provider = registry.get(model.api);
	if (!provider) throw new Error(`No API provider registered for api: ${model.api}`);
	return provider.stream(model, context).result();
}

function registerFaux(api, provider) {
	registerApiProvider({
		api,
		stream(model) {
			return {
				async result() {
					return { api, provider, model: model.id, role: "assistant", text: api };
				},
			};
		},
	});
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
registerFaux("faux:empty", "faux");
const empty = [];
for (const [input, context] of emptyContexts) {
	const response = await complete({ api: "faux:empty", id: "empty-model" }, context);
	empty.push({ input, role: response.role, contentDefined: response.text !== undefined, error: false });
}

for (const api of apis) registerFaux(api, `oracle-${api}`);
const dispatch = [];
for (const api of apis) dispatch.push(await complete({ api, id: "oracle-model" }, { messages: [] }));

process.stdout.write(JSON.stringify({ empty, dispatch }));
