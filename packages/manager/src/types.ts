import type { Json, MachineManifest } from '@zf/core';

export interface ProcessRequest {
  readonly command: 'git' | 'pnpm';
  readonly args: readonly string[];
  readonly cwd: string;
  /** Deliberately allowlisted; never merge the caller's environment into this environment. */
  readonly env: NodeJS.ProcessEnv;
}
export interface ProcessRunner {
  readonly run: (request: ProcessRequest) => Promise<string>;
}
export interface MachineManagerOptions {
  /** Explicit .zedflow root. No default home directory is consulted. */
  readonly directory: string;
  readonly process?: ProcessRunner;
  /** Registry URL is an explicit transport seam, never inherited from user/source configuration. */
  readonly registry?: string;
}
export interface InstallReceipt {
  readonly id: string;
  readonly source: string;
  /** Exact registry version, full Git commit, or local source digest. */
  readonly resolved: string;
  readonly installId: string;
  /** Relative to the explicit manager directory. */
  readonly destination: string;
  readonly sourceIntegrity: string;
  readonly installedIntegrity: string;
  readonly dependencyIntegrity: string;
}
export interface Source {
  readonly kind: 'local' | 'git' | 'npm';
  readonly normalized: string;
  readonly location: string;
  readonly ref: string;
}
export interface LockedSource {
  readonly source: Source;
  readonly resolved: string;
  readonly sourceIntegrity: string;
  readonly dependencyIntegrity: string;
  readonly dependencyManifest: Readonly<Record<string, Json>>;
  readonly dependencyLock: string | null;
  /** Runtime node_modules path, relative to the immutable snapshot. */
  readonly dependencyPath: string | null;
}
export interface ManagerLock {
  readonly schemaVersion: 1;
  /** Manager-created namespace directories, relative to machines/. Preexisting directories are not owned. */
  readonly groupDirectories?: readonly string[];
  readonly sources: Readonly<Record<string, LockedSource>>;
  readonly receipts: readonly InstallReceipt[];
}
export interface DiscoveredMachine {
  readonly id: string;
  readonly relativeDirectory: string;
  readonly manifest: MachineManifest;
}
export interface PreparedSource {
  readonly installId: string;
  readonly directory: string;
  readonly locked: LockedSource;
  readonly machines: readonly DiscoveredMachine[];
}
