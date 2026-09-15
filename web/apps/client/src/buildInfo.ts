export interface BuildInfo { component:'client'|'daemon';version:string;buildId:string;protocol:number;storageEpoch:number;revision?:string|null;target?:string }
declare const __ZEDFLOW_CLIENT_BUILD__: BuildInfo
// Pure authoring helpers are also imported by Node fixture tests, outside Vite.
// An unbundled consumer must never advertise a compatible application build.
export const CLIENT_BUILD:BuildInfo=typeof __ZEDFLOW_CLIENT_BUILD__==='undefined'
  ? {component:'client',version:'non compilé',buildId:'unbundled',protocol:0,storageEpoch:0}
  : __ZEDFLOW_CLIENT_BUILD__
