import assert from "node:assert/strict";
import test from "node:test";

import type { ServerMessage } from "./generated/protocol";
import { RuntimeFrameStager } from "./frame-stager";
import { RuntimeUiStore } from "./store";

function valueFrame(value: number): ArrayBuffer {
  const buffer = new ArrayBuffer(30);
  const bytes = new Uint8Array(buffer);
  bytes.set([0x47, 0x56, 0x46, 0x31]);
  const view = new DataView(buffer);
  view.setUint16(4, 1, true);
  view.setUint32(6, 1, true);
  view.setUint32(10, 2, true);
  view.setUint32(14, 1, true);
  view.setUint32(18, 9, true);
  view.setFloat64(22, value, true);
  return buffer;
}

test("many messages stage into one coherent frame with latest-wins previews and values", () => {
  const callbacks: ((timestamp: number) => void)[] = [];
  const store = new RuntimeUiStore();
  const stager = new RuntimeFrameStager(store, (callback) => {
    callbacks.push(callback);
    return callbacks.length;
  });
  for (let sequence = 0; sequence < 1_000; sequence += 1) {
    const message: ServerMessage = {
      plane: "observation",
      payload: {
        kind: "preview",
        payload: {
          sequence,
          changes: [
            {
              key: { scope: "runtime", entity: "node", field: "value" },
              value: { kind: "integer", value: sequence },
            },
          ],
        },
      },
    };
    stager.stageMessage(message, 1);
    stager.stageBinary(valueFrame(sequence), 1);
  }
  assert.equal(callbacks.length, 1);
  callbacks[0]!(10);

  assert.equal(store.frameRevision, 1);
  assert.equal(store.previews.size, 1);
  assert.equal(store.values.get(9), 999);
  assert.equal(stager.metrics.previewReplacements, 999);
  assert.equal(stager.metrics.binaryReplacements, 999);
});
