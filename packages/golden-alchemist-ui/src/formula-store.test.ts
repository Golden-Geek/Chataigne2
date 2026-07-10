import assert from "node:assert/strict";
import test from "node:test";

import { FormulaStore } from "./formula-store";
import type { FormulaId, SurfaceItemId } from "./types";

const formulaId = "formula" as FormulaId;
const outputId = "result" as SurfaceItemId;

test("runtime deltas preserve keyed collection identity and isolate output revisions", () => {
  const store = new FormulaStore({
    formulaId,
    revision: 0,
    catalog: [],
    inputs: [],
    outputs: [
      {
        id: outputId,
        label: "Result",
        source: { node: "node" as never, port: "port" as never },
        valueType: "float",
      },
    ],
  });
  const outputs = store.outputs;

  store.apply({
    before: 0,
    after: 1,
    changes: [{ kind: "runtime-output", outputId, value: 42 }],
  });

  assert.equal(store.outputs, outputs);
  assert.equal(store.runtimeOutputs.get(outputId), 42);
  assert.equal(store.outputRevision, 1);
  assert.equal(store.surfaceRevision, 0);
});

test("catalog changes do not invalidate formula surface state", () => {
  const store = new FormulaStore({
    formulaId,
    revision: 3,
    catalog: [],
    inputs: [],
    outputs: [],
  });
  store.apply({
    before: 3,
    after: 4,
    changes: [
      {
        kind: "catalog-upserted",
        item: { id: formulaId, name: "Formula", description: "", tags: [], builtIn: true },
      },
    ],
  });

  assert.equal(store.catalogRevision, 1);
  assert.equal(store.surfaceRevision, 0);
  assert.equal(store.outputRevision, 0);
});
