import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { modelCatalogQuerySchema, type ModelCatalogQuery } from './queries.js';
import { modelCatalogSchema, codexStatusSchema, type ModelCatalog, type CodexStatus } from './results.js';
export function createModelsClient(transport: Transport) {
    return {
        async list(input: ModelCatalogQuery = {}, signal?: AbortSignal): Promise<ModelCatalog> {
            const query = validateInput('models.list', modelCatalogQuerySchema, input);
            return transport.json({ operation: 'models.list', method: 'GET', path: 'models', query, ...(signal ? { signal } : {}) }, modelCatalogSchema);
        },
        codexStatus(signal?: AbortSignal): Promise<CodexStatus> {
            return transport.json({ operation: 'models.codexStatus', method: 'GET', path: 'auth/codex', ...(signal ? { signal } : {}) }, codexStatusSchema);
        },
    };
}
export type ModelsClient = ReturnType<typeof createModelsClient>;
