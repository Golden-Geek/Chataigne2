import assert from "node:assert/strict";
import test from "node:test";

import type { GraphNodeId } from "@golden/graph-ui";

import { StatechartStore } from "./statechart-store";

const idle = "idle" as GraphNodeId;
const active = "active" as GraphNodeId;

test("runtime transition deltas update stable active-state identity", () => {
  const store = new StatechartStore({ revision: 2, activeStates: [idle] });
  const identity = store.activeStates;
  store.apply({
    before: 2,
    after: 3,
    changes: [
      { kind: "state-exited", stateId: idle },
      { kind: "state-entered", stateId: active },
    ],
  });
  assert.equal(store.activeStates, identity);
  assert.deepEqual([...store.activeStates], [active]);
  assert.equal(store.activeRevision, 1);
});
