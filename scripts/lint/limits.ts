export interface Limits {
  readonly fileLines: number;
  readonly functionLines: number;
  readonly ignoreDescribe?: boolean;
  readonly params: number;
  readonly statements?: number;
  readonly complexity?: number;
  readonly depth?: number;
  readonly dependencies?: number;
  readonly classesPerFile?: number;
  readonly moduleFunctions?: number;
  readonly variables?: number;
  readonly classLines?: number;
  readonly classMethods?: number;
  readonly machineConfigFunctionLines?: number;
}
const source: Limits = {
  fileLines: 200,
  functionLines: 30,
  params: 4,
  statements: 12,
  complexity: 8,
  depth: 3,
  dependencies: 10,
  classesPerFile: 1,
  moduleFunctions: 8,
  variables: 8,
  classLines: 150,
  classMethods: 10,
};
export const limits = {
  source,
  test: { fileLines: 400, functionLines: 50, params: 4, ignoreDescribe: true },
  machine: { ...source, functionLines: 80, machineConfigFunctionLines: 3 },
} satisfies Record<string, Limits>;
