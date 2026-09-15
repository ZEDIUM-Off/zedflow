import { contextProgramSchema, type ContextProgram } from '../context/model.js';
import { z } from 'zod';
import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { jsonValueSchema } from '../core/json.js';
import * as m from './model.js';
import * as c from './commands.js';
import * as q from './queries.js';
import * as r from './results.js';
export function createRunsClient(transport: Transport) {
    return {
        async offerRtc(id: string, input: c.RtcOfferInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RtcAnswer> {
            const key = validateInput("runs.offerRtc", z.string().min(1), id);
            const parsed = validateInput("runs.offerRtc", c.rtcOfferInputSchema, input);
            const body = validateInput("runs.offerRtc", jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput("runs.offerRtc", q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: "runs.offerRtc", method: "POST", path: `runs/${encodeURIComponent(key)}/rtc`, body, query, ...(signal ? { signal } : {}) }, r.rtcAnswerSchema);
        },
        async activity(id: string, occurrenceId: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.NodeActivity> {
            const idKey = validateInput('runs.activity', z.string().min(1), id);
            const occurrenceIdKey = validateInput('runs.activity', z.string().min(1), occurrenceId);
            const query = validateInput('runs.activity', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.activity', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/activities/${encodeURIComponent(occurrenceIdKey)}`, query, ...(signal ? { signal } : {}) }, m.nodeActivitySchema);
        },
        async boundary(id: string, occurrenceId: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.PassageBoundary> {
            const idKey = validateInput('runs.boundary', z.string().min(1), id);
            const occurrenceIdKey = validateInput('runs.boundary', z.string().min(1), occurrenceId);
            const query = validateInput('runs.boundary', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.boundary', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/boundaries/${encodeURIComponent(occurrenceIdKey)}`, query, ...(signal ? { signal } : {}) }, r.passageBoundarySchema);
        },
        async tool(id: string, callId: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ToolActivity> {
            const idKey = validateInput('runs.tool', z.string().min(1), id);
            const callIdKey = validateInput('runs.tool', z.string().min(1), callId);
            const query = validateInput('runs.tool', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.tool', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/tools/${encodeURIComponent(callIdKey)}`, query, ...(signal ? { signal } : {}) }, m.toolActivitySchema);
        },
        async toolOutput(id: string, callId: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<Uint8Array> {
            const idKey = validateInput('runs.toolOutput', z.string().min(1), id);
            const callIdKey = validateInput('runs.toolOutput', z.string().min(1), callId);
            const query = validateInput('runs.toolOutput', q.workspaceQuerySchema, queryInput);
            return transport.bytes({ operation: 'runs.toolOutput', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/tools/${encodeURIComponent(callIdKey)}/output`, query, ...(signal ? { signal } : {}) });
        },
        async contextSnapshot(id: string, invocationId: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.ContextSnapshot> {
            const idKey = validateInput('runs.contextSnapshot', z.string().min(1), id);
            const invocationIdKey = validateInput('runs.contextSnapshot', z.string().min(1), invocationId);
            const query = validateInput('runs.contextSnapshot', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.contextSnapshot', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/context/${encodeURIComponent(invocationIdKey)}`, query, ...(signal ? { signal } : {}) }, m.contextSnapshotSchema);
        },
        async request(id: string, invocationId: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RequestDetail> {
            const idKey = validateInput('runs.request', z.string().min(1), id);
            const invocationIdKey = validateInput('runs.request', z.string().min(1), invocationId);
            const query = validateInput('runs.request', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.request', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/requests/${encodeURIComponent(invocationIdKey)}`, query, ...(signal ? { signal } : {}) }, r.requestDetailSchema);
        },
        async requestRaw(id: string, invocationId: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<Uint8Array> {
            const idKey = validateInput('runs.requestRaw', z.string().min(1), id);
            const invocationIdKey = validateInput('runs.requestRaw', z.string().min(1), invocationId);
            const query = validateInput('runs.requestRaw', q.workspaceQuerySchema, queryInput);
            return transport.bytes({ operation: 'runs.requestRaw', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/requests/${encodeURIComponent(invocationIdKey)}/raw`, query, ...(signal ? { signal } : {}) });
        },
        async contextWindows(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.WindowReference[]> {
            const idKey = validateInput('runs.contextWindows', z.string().min(1), id);
            const query = validateInput('runs.contextWindows', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.contextWindows', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/context-windows`, query, ...(signal ? { signal } : {}) }, z.array(m.windowReferenceSchema));
        },
        async contextWindow(id: string, queryInput: q.ContextWindowQuery, signal?: AbortSignal): Promise<r.ContextWindowDetail> {
            const idKey = validateInput('runs.contextWindow', z.string().min(1), id);
            const query = validateInput('runs.contextWindow', q.contextWindowQuerySchema, queryInput);
            return transport.json({ operation: 'runs.contextWindow', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/context-window`, query, ...(signal ? { signal } : {}) }, r.contextWindowDetailSchema);
        },
        async contextProgram(id: string, queryInput: q.ContextProgramQuery, signal?: AbortSignal): Promise<ContextProgram> {
            const idKey = validateInput('runs.contextProgram', z.string().min(1), id);
            const query = validateInput('runs.contextProgram', q.contextProgramQuerySchema, queryInput);
            return transport.json({ operation: 'runs.contextProgram', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/context-program`, query, ...(signal ? { signal } : {}) }, contextProgramSchema);
        },
        async state(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunStateDetail> {
            const idKey = validateInput('runs.state', z.string().min(1), id);
            const query = validateInput('runs.state', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.state', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/state`, query, ...(signal ? { signal } : {}) }, r.runStateDetailSchema);
        },
        async metrics(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunMetrics> {
            const idKey = validateInput('runs.metrics', z.string().min(1), id);
            const query = validateInput('runs.metrics', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.metrics', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/metrics`, query, ...(signal ? { signal } : {}) }, r.runMetricsSchema);
        },
        async events(id: string, queryInput: q.RunCursorQuery = {}, signal?: AbortSignal): Promise<r.RunEventPage> {
            const idKey = validateInput('runs.events', z.string().min(1), id);
            const query = validateInput('runs.events', q.runCursorQuerySchema, queryInput);
            return transport.json({ operation: 'runs.events', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/event-history`, query, ...(signal ? { signal } : {}) }, r.runEventPageSchema);
        },
        async timeline(id: string, queryInput: q.TimelineQuery = {}, signal?: AbortSignal): Promise<r.TimelinePage> {
            const idKey = validateInput('runs.timeline', z.string().min(1), id);
            const query = validateInput('runs.timeline', q.timelineQuerySchema, queryInput);
            return transport.json({ operation: 'runs.timeline', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/timeline`, query, ...(signal ? { signal } : {}) }, r.timelinePageSchema);
        },
        async flowSource(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.ExecutedSource> {
            const idKey = validateInput('runs.flowSource', z.string().min(1), id);
            const query = validateInput('runs.flowSource', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.flowSource', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/flow-source`, query, ...(signal ? { signal } : {}) }, r.executedSourceSchema);
        },
        async definition(id: string, queryInput: q.DefinitionQuery = {}, signal?: AbortSignal): Promise<r.ExecutedDefinition> {
            const idKey = validateInput('runs.definition', z.string().min(1), id);
            const query = validateInput('runs.definition', q.definitionQuerySchema, queryInput);
            return transport.json({ operation: 'runs.definition', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/definition`, query, ...(signal ? { signal } : {}) }, r.executedDefinitionSchema);
        },
        async revisions(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunRevisions> {
            const idKey = validateInput('runs.revisions', z.string().min(1), id);
            const query = validateInput('runs.revisions', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.revisions', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/revisions`, query, ...(signal ? { signal } : {}) }, r.runRevisionsSchema);
        },
        async previewSource(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.PreviewSource> {
            const idKey = validateInput('runs.previewSource', z.string().min(1), id);
            const query = validateInput('runs.previewSource', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.previewSource', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/preview-source`, query, ...(signal ? { signal } : {}) }, r.previewSourceSchema);
        },
        async patchContextWindow(id: string, input: c.PatchContextWindowInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.ContextWindowDetail> {
            const key = validateInput('runs.patchContextWindow', z.string().min(1), id);
            const parsed = validateInput('runs.patchContextWindow', c.patchContextWindowInputSchema, input);
            const body = validateInput('runs.patchContextWindow', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput('runs.patchContextWindow', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.patchContextWindow', method: 'PATCH', path: `runs/${encodeURIComponent(key)}/context-window`, body, query, ...(signal ? { signal } : {}) }, r.contextWindowDetailSchema);
        },
        async selectContextWindow(id: string, input: c.SelectContextWindowInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<c.SelectContextWindowInput> {
            const key = validateInput('runs.selectContextWindow', z.string().min(1), id);
            const parsed = validateInput('runs.selectContextWindow', c.selectContextWindowInputSchema, input);
            const body = validateInput('runs.selectContextWindow', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput('runs.selectContextWindow', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.selectContextWindow', method: 'POST', path: `runs/${encodeURIComponent(key)}/context-window/select`, body, query, ...(signal ? { signal } : {}) }, c.selectContextWindowInputSchema);
        },
        async start(input: c.StartRunInput, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const parsed = validateInput('runs.start', c.startRunInputSchema, input);
            const body = validateInput('runs.start', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'runs.start', method: 'POST', path: `runs`, body, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async preview(input: c.PreviewRunInput, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const parsed = validateInput('runs.preview', c.previewRunInputSchema, input);
            const body = validateInput('runs.preview', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            return transport.json({ operation: 'runs.preview', method: 'POST', path: `runs/preview`, body, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async list(queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.RunSummary[]> {
            const query = validateInput('runs.list', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.list', method: 'GET', path: `runs`, query, ...(signal ? { signal } : {}) }, z.array(m.runSummarySchema));
        },
        async read(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<m.Run> {
            const idKey = validateInput('runs.read', z.string().min(1), id);
            const query = validateInput('runs.read', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.read', method: 'GET', path: `runs/${encodeURIComponent(idKey)}`, query, ...(signal ? { signal } : {}) }, m.runSchema);
        },
        async snapshot(id: string, queryInput: q.RunCursorQuery = {}, signal?: AbortSignal): Promise<r.RunSync> {
            const idKey = validateInput('runs.snapshot', z.string().min(1), id);
            const query = validateInput('runs.snapshot', q.runCursorQuerySchema, queryInput);
            return transport.json({ operation: 'runs.snapshot', method: 'GET', path: `runs/${encodeURIComponent(idKey)}/snapshot`, query, ...(signal ? { signal } : {}) }, r.runSyncSchema);
        },
        async answer(id: string, input: c.AnswerRunInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const idKey = validateInput('runs.answer', z.string().min(1), id);
            const parsed = validateInput('runs.answer', c.answerRunInputSchema, input);
            const body = validateInput('runs.answer', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput('runs.answer', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.answer', method: 'POST', path: `runs/${encodeURIComponent(idKey)}/answer`, body, query, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async rename(id: string, input: c.RenameRunInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const idKey = validateInput('runs.rename', z.string().min(1), id);
            const parsed = validateInput('runs.rename', c.renameRunInputSchema, input);
            const body = validateInput('runs.rename', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput('runs.rename', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.rename', method: 'PATCH', path: `runs/${encodeURIComponent(idKey)}`, body, query, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async selectModel(id: string, input: c.SelectRunModelInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const idKey = validateInput('runs.selectModel', z.string().min(1), id);
            const parsed = validateInput('runs.selectModel', c.selectRunModelInputSchema, input);
            const body = validateInput('runs.selectModel', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput('runs.selectModel', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.selectModel', method: 'PATCH', path: `runs/${encodeURIComponent(idKey)}/models`, body, query, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async activateCapability(id: string, input: c.ActivateCapabilityInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const idKey = validateInput('runs.activateCapability', z.string().min(1), id);
            const parsed = validateInput('runs.activateCapability', c.activateCapabilityInputSchema, input);
            const body = validateInput('runs.activateCapability', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput('runs.activateCapability', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.activateCapability', method: 'POST', path: `runs/${encodeURIComponent(idKey)}/capabilities`, body, query, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async queueMessage(id: string, input: c.QueueMessageInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const idKey = validateInput('runs.queueMessage', z.string().min(1), id);
            const parsed = validateInput('runs.queueMessage', c.queueMessageInputSchema, input);
            const body = validateInput('runs.queueMessage', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput('runs.queueMessage', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.queueMessage', method: 'POST', path: `runs/${encodeURIComponent(idKey)}/messages`, body, query, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async resume(id: string, input: c.ResumeRunInput, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const idKey = validateInput('runs.resume', z.string().min(1), id);
            const parsed = validateInput('runs.resume', c.resumeRunInputSchema, input);
            const body = validateInput('runs.resume', jsonValueSchema, JSON.parse(JSON.stringify(parsed)));
            const query = validateInput('runs.resume', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.resume', method: 'POST', path: `runs/${encodeURIComponent(idKey)}/resume`, body, query, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async abort(id: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const idKey = validateInput('runs.abort', z.string().min(1), id);
            const query = validateInput('runs.abort', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.abort', method: 'POST', path: `runs/${encodeURIComponent(idKey)}/abort`, query, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
        async removeMessage(id: string, messageId: string, queryInput: q.WorkspaceQuery = {}, signal?: AbortSignal): Promise<r.RunAcknowledgement> {
            const idKey = validateInput('runs.removeMessage', z.string().min(1), id);
            const messageIdKey = validateInput('runs.removeMessage', z.string().min(1), messageId);
            const query = validateInput('runs.removeMessage', q.workspaceQuerySchema, queryInput);
            return transport.json({ operation: 'runs.removeMessage', method: 'DELETE', path: `runs/${encodeURIComponent(idKey)}/messages/${encodeURIComponent(messageIdKey)}`, query, ...(signal ? { signal } : {}) }, r.runAcknowledgementSchema);
        },
    };
}
export type RunsClient = ReturnType<typeof createRunsClient>;
