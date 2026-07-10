import type { GraphNodeId } from "@golden/graph-ui";

import type { StatechartRuntimeDelta, StatechartSnapshot } from "./types";

export class StatechartRevisionConflict extends Error {}

export class StatechartStore {
  readonly activeStates = new Set<GraphNodeId>();
  revision: number;
  activeRevision = 0;

  constructor(snapshot: StatechartSnapshot) {
    this.revision = snapshot.revision;
    for (const state of snapshot.activeStates) this.activeStates.add(state);
  }

  apply(delta: StatechartRuntimeDelta): void {
    if (delta.before !== this.revision || delta.after <= delta.before) {
      throw new StatechartRevisionConflict(
        `expected statechart revision ${this.revision}, received ${delta.before} -> ${delta.after}`,
      );
    }
    let changed = false;
    for (const change of delta.changes) {
      if (change.kind === "state-entered") {
        const size = this.activeStates.size;
        this.activeStates.add(change.stateId);
        changed ||= this.activeStates.size !== size;
      } else {
        changed = this.activeStates.delete(change.stateId) || changed;
      }
    }
    this.revision = delta.after;
    if (changed) this.activeRevision += 1;
  }
}
