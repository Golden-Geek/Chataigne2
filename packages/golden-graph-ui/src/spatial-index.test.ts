import assert from "node:assert/strict";
import test from "node:test";

import { SpatialIndex } from "./spatial-index";
import type { GraphNodeId } from "./types";

test("visible queries inspect nearby cells rather than the full graph", () => {
  const index = new SpatialIndex(100);
  for (let value = 0; value < 100_000; value += 1) {
    index.upsert(`node-${value}` as GraphNodeId, {
      x: (value % 1_000) * 100,
      y: Math.floor(value / 1_000) * 100,
      width: 20,
      height: 20,
    });
  }
  const visible = index.query({ x: 450, y: 450, width: 200, height: 200 });
  assert.ok(visible.length > 0);
  assert.ok(visible.length <= 9, `expected at most 9 nearby nodes, received ${visible.length}`);
});

test("updates remove stale cell membership", () => {
  const index = new SpatialIndex(100);
  const node = "node" as GraphNodeId;
  index.upsert(node, { x: 0, y: 0, width: 20, height: 20 });
  index.upsert(node, { x: 500, y: 500, width: 20, height: 20 });
  assert.deepEqual(index.query({ x: 0, y: 0, width: 20, height: 20 }), []);
  assert.deepEqual(index.query({ x: 500, y: 500, width: 20, height: 20 }), [node]);
});
