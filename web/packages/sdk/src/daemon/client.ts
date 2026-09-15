import { rtcConfigSchema, type RtcConfig } from './model.js';
import type { Transport } from '../core/transport.js';
import { validateInput } from '../core/schema.js';
import { daemonVersionSchema, daemonHealthSchema, capabilityInventorySchema, type DaemonVersion, type DaemonHealth, type CapabilityInventory } from './model.js';
import { applyUpdateSchema, type ApplyUpdateInput } from './commands.js';
import { updateAcknowledgementSchema, type UpdateAcknowledgement } from './results.js';
export function createDaemonClient(transport: Transport) {
    return {
        rtcConfig(signal?: AbortSignal): Promise<RtcConfig> {
            return transport.json({ operation: "daemon.rtcConfig", method: "GET", path: "rtc/config", ...(signal ? { signal } : {}) }, rtcConfigSchema);
        },
        health(signal?: AbortSignal): Promise<DaemonHealth> {
            return transport.json({ operation: 'daemon.health', method: 'GET', path: 'health', ...(signal ? { signal } : {}) }, daemonHealthSchema);
        },
        capabilities(signal?: AbortSignal): Promise<CapabilityInventory> {
            return transport.json({ operation: 'daemon.capabilities', method: 'GET', path: 'capabilities', ...(signal ? { signal } : {}) }, capabilityInventorySchema);
        },
        /** HTTP 404 remains distinguishable from a disconnected or invalid daemon. */
        version(signal?: AbortSignal): Promise<DaemonVersion> {
            return transport.json({ operation: 'daemon.version', method: 'GET', path: 'version', ...(signal ? { signal } : {}) }, daemonVersionSchema);
        },
        async applyUpdate(input: ApplyUpdateInput, signal?: AbortSignal): Promise<UpdateAcknowledgement> {
            const body = validateInput('daemon.applyUpdate', applyUpdateSchema, input);
            return transport.json({ operation: 'daemon.applyUpdate', method: 'POST', path: 'updates/apply', body, ...(signal ? { signal } : {}) }, updateAcknowledgementSchema);
        },
    };
}
export type DaemonClient = ReturnType<typeof createDaemonClient>;
