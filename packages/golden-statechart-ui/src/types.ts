import type { GraphNodeId } from "@golden/graph-ui";

export interface StatechartSnapshot {
  revision: number;
  activeStates: readonly GraphNodeId[];
}

export type StatechartRuntimeChange =
  | { kind: "state-entered"; stateId: GraphNodeId }
  | { kind: "state-exited"; stateId: GraphNodeId };

export interface StatechartRuntimeDelta {
  before: number;
  after: number;
  changes: readonly StatechartRuntimeChange[];
}
