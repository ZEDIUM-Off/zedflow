// Executable differential oracle over the frozen Pi builtin compat providers.
// Provider SDKs are replaced below the provider implementations; conversions,
// payload hooks, stream parsing, and result normalization remain frozen Pi code.

import { register } from "node:module";

const loader = `
const modules = new Map([
 ["typebox", "const noop = new Proxy(() => ({}), { get: () => noop }); export const Type = noop;"],
 ["typebox/compile", "export const Compile = () => ({ Check: () => true, Errors: () => [] });"],
 ["typebox/value", "export const Value = {};"],
 ["partial-json", "export const parse = JSON.parse;"],
 ["@anthropic-ai/sdk", \`
   const sse = p => [
     ["message_start",{type:"message_start",message:{id:"a-"+p.model,usage:{input_tokens:1,output_tokens:0}}}],
     ["content_block_start",{type:"content_block_start",index:0,content_block:{type:"text",text:""}}],
     ["content_block_delta",{type:"content_block_delta",index:0,delta:{type:"text_delta",text:"reply:"+p.model}}],
     ["content_block_stop",{type:"content_block_stop",index:0}],
     ["message_delta",{type:"message_delta",delta:{stop_reason:"end_turn"},usage:{output_tokens:1}}],
     ["message_stop",{type:"message_stop"}],
   ].map(([e,d]) => "event: "+e+String.fromCharCode(10)+"data: "+JSON.stringify(d)+String.fromCharCode(10,10)).join("");
   export default class Anthropic { constructor() { this.messages={create:p=>({asResponse:async()=>new Response(sse(p),{status:200,headers:{"content-type":"text/event-stream"}})})}; } }
 \`],
 ["openai", \`
   const responseEvents = p => [
     {type:"response.output_item.added",output_index:0,item:{type:"message",id:"m-"+p.model,status:"in_progress",role:"assistant",content:[]}},
     {type:"response.output_text.delta",output_index:0,content_index:0,delta:"reply:"+p.model},
     {type:"response.output_item.done",output_index:0,item:{type:"message",id:"m-"+p.model,status:"completed",role:"assistant",content:[{type:"output_text",text:"reply:"+p.model,annotations:[]}]}},
     {type:"response.completed",response:{id:"r-"+p.model,status:"completed",output:[],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}},
   ];
   const iterable = xs => ({async *[Symbol.asyncIterator](){yield* xs}});
   const wrapped = xs => ({withResponse:async()=>({data:iterable(xs),response:new Response("",{status:200})})});
   export default class OpenAI { constructor(){ this.chat={completions:{create:p=>wrapped([{id:"c-"+p.model,model:p.model,choices:[{delta:{content:"reply:"+p.model},finish_reason:"stop"}],usage:{prompt_tokens:1,completion_tokens:1,total_tokens:2}}])}}; this.responses={create:p=>wrapped(responseEvents(p))}; } }
   export class AzureOpenAI extends OpenAI {}
 \`],
 ["@google/genai", \`
   export const ResourceScope={PROJECT:"PROJECT"}; export const ThinkingLevel={MINIMAL:"MINIMAL",LOW:"LOW",MEDIUM:"MEDIUM",HIGH:"HIGH"}; export const FinishReason={STOP:"STOP",MAX_TOKENS:"MAX_TOKENS"}; export const FunctionCallingConfigMode={AUTO:"AUTO",NONE:"NONE",ANY:"ANY"};
   export class GoogleGenAI { constructor(){ this.models={generateContentStream:async p=>({
     async *[Symbol.asyncIterator](){ yield {responseId:"g-"+p.model,candidates:[{content:{parts:[{text:"reply:"+p.model}]},finishReason:"STOP"}],usageMetadata:{promptTokenCount:1,candidatesTokenCount:1,totalTokenCount:2}}; }
   })}; } }
 \`],
 ["@mistralai/mistralai", \`
   export class Mistral { constructor(){ this.chat={stream:async p=>({async *[Symbol.asyncIterator](){yield {data:{id:"m-"+p.model,choices:[{delta:{content:"reply:"+p.model},finishReason:"stop"}],usage:{promptTokens:1,completionTokens:1,totalTokens:2}}}}})}; } }
 \`],
 ["@aws-sdk/client-bedrock-runtime", \`
   export const ConversationRole={ASSISTANT:"assistant",USER:"user"}; export const StopReason={END_TURN:"end_turn",MAX_TOKENS:"max_tokens",TOOL_USE:"tool_use"}; export const CachePointType={DEFAULT:"default"}; export const CacheTTL={FIVE_MINUTES:"5m",ONE_HOUR:"1h"}; export const ImageFormat={JPEG:"jpeg",PNG:"png",GIF:"gif",WEBP:"webp"}; export const ToolChoice={}; export const ToolResultStatus={SUCCESS:"success",ERROR:"error"};
   export class BedrockRuntimeServiceException extends Error {}
   export class ConverseStreamCommand { constructor(input){this.input=input} }
   export class BedrockRuntimeClient { constructor(){this.middlewareStack={add(){}}} async send(c){ const p=c.input; return {
     $metadata:{httpStatusCode:200}, stream:{ async *[Symbol.asyncIterator](){
       yield {messageStart:{role:"assistant"}}; yield {contentBlockDelta:{contentBlockIndex:0,delta:{text:"reply:"+p.modelId}}}; yield {contentBlockStop:{contentBlockIndex:0}}; yield {messageStop:{stopReason:"end_turn"}}; yield {metadata:{usage:{inputTokens:1,outputTokens:1,totalTokens:2}}};
     }}
   }; } }
 \`],
 ["@smithy/node-http-handler", "export class NodeHttpHandler {}"],
 ["http-proxy-agent", "export class HttpProxyAgent {}"],
 ["https-proxy-agent", "export class HttpsProxyAgent {}"],
]);
export async function resolve(specifier, context, nextResolve) {
 if (modules.has(specifier)) return {url:"oracle:"+encodeURIComponent(specifier),shortCircuit:true};
 return nextResolve(specifier,context);
}
export async function load(url, context, nextLoad) {
 if (url.startsWith("oracle:")) return {format:"module",source:modules.get(decodeURIComponent(url.slice(7))),shortCircuit:true};
 return nextLoad(url,context);
}`;
register(`data:text/javascript,${encodeURIComponent(loader)}`, import.meta.url);

const compat = await import("../../../../references/pi/packages/ai/src/compat.ts");
const apis = [
	"anthropic-messages", "openai-completions", "openai-responses",
	"openai-codex-responses", "azure-openai-responses", "google-generative-ai",
	"google-vertex", "mistral-conversations", "bedrock-converse-stream",
];

function stable(value) {
	if (Array.isArray(value)) return value.map(stable);
	if (!value || typeof value !== "object") return value;
	return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => [k, stable(v)]));
}
function observe(response, payload) {
	return stable({
		api: response.api, provider: response.provider, model: response.model,
		role: response.role, stopReason: response.stopReason,
		text: response.content.find((block) => block.type === "text")?.text,
		request: payload,
	});
}

// Codex deliberately uses fetch rather than the OpenAI SDK.
let transportPayload;
globalThis.fetch = async () => {
	const p = transportPayload;
	const events = [
		{type:"response.output_item.added",output_index:0,item:{type:"message",id:"m-"+p.model,status:"in_progress",role:"assistant",content:[]}},
		{type:"response.output_text.delta",output_index:0,content_index:0,delta:"reply:"+p.model},
		{type:"response.output_item.done",output_index:0,item:{type:"message",id:"m-"+p.model,status:"completed",role:"assistant",content:[{type:"output_text",text:"reply:"+p.model,annotations:[]}]}},
		{type:"response.completed",response:{id:"r-"+p.model,status:"completed",output:[],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}},
	];
	return new Response(events.map(e=>`data: ${JSON.stringify(e)}\n\n`).join("")+"data: [DONE]\n\n",{status:200,headers:{"content-type":"text/event-stream"}});
};

const context = {
	systemPrompt: "oracle-system",
	messages: [
		{role:"user",content:"hello oracle"},
		{role:"assistant",content:[{type:"text",text:"prior"}],api:"oracle",provider:"oracle",model:"oracle",stopReason:"stop",usage:{input:0,output:0,cacheRead:0,cacheWrite:0,totalTokens:0,cost:{input:0,output:0,cacheRead:0,cacheWrite:0,total:0}},timestamp:1},
		{role:"user",content:"continue"},
	],
};
const observations = [];
const codexKey = `x.${Buffer.from(JSON.stringify({"https://api.openai.com/auth":{chatgpt_account_id:"oracle"}})).toString("base64url")}.x`;
for (const api of apis) {
	const model = compat.getProviders().flatMap((provider) => compat.getModels(provider)).find((candidate) => candidate.api === api);
	if (!model) throw new Error(`missing builtin model for ${api}`);
	if (api === "azure-openai-responses") model.baseUrl = "https://oracle.openai.azure.com/openai/v1";
	let payload;
	const response = await compat.complete(model, context, {
		apiKey: api === "openai-codex-responses" ? codexKey : "oracle-key",
		transport: api === "openai-codex-responses" ? "sse" : undefined,
		env:{GOOGLE_CLOUD_PROJECT:"oracle",GOOGLE_CLOUD_LOCATION:"global",AWS_REGION:"us-east-1",AWS_BEDROCK_SKIP_AUTH:"1"},
		onPayload(value) { payload = structuredClone(value); transportPayload = payload; },
	});
	observations.push(observe(response, payload));
}
process.stdout.write(JSON.stringify(observations));
