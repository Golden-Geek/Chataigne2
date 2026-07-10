import assert from "node:assert/strict";
import test from "node:test";

import { GraphRevisionConflict, GraphStore } from "./store";
import type { GraphId, GraphNodeId, GraphSnapshot } from "./types";

const id = <T extends string>(value: string) => value as T;

test("one-node deltas retain collection and untouched-record identity", () => {
  const first = { id: id<GraphNodeId>("first"), data: { value: 1 } };
  const second = { id: id<GraphNodeId>("second"), data: { value: 2 } };
  const snapshot: GraphSnapshot<{ value: number }, never> = {
    graphId: id<GraphId>("graph"),
    revision: 4,
    nodes: [first, second],
    edges: [],
    geometry: {},
  };
  const store = new GraphStore(snapshot);
  const nodes = store.nodes;
  store.apply({
    before: 4,
    after: 5,
    changes: [{ kind: "node-replaced", node: { id: first.id, data: { value: 3 } } }],
  });
  assert.equal(store.nodes, nodes);
  assert.equal(store.nodes.get(second.id), second);
  assert.equal(store.nodes.get(first.id)?.data.value, 3);
  assert.equal(store.topologyRevision, 1);
});

test("revision mismatches request resync instead of applying partial state", () => {
  const store = new GraphStore({
    graphId: id<GraphId>("graph"),
    revision: 2,
    nodes: [],
    edges: [],
    geometry: {},
  });
  assert.throws(
    () => store.apply({ before: 1, after: 3, changes: [] }),
    GraphRevisionConflict,
  );
  assert.equal(store.graphRevision, 2);
});
