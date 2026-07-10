import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";

import { build } from "esbuild";
import { chromium } from "playwright";

test("real Chromium commits a preview burst in one animation frame", async () => {
  const source = await build({
    stdin: {
      contents: `
        import { RuntimeFrameStager } from "./frame-stager.ts";
        import { RuntimeUiStore } from "./store.ts";
        window.GoldenRuntimeTest = { RuntimeFrameStager, RuntimeUiStore };
      `,
      resolveDir: path.resolve("packages/golden-runtime-client/src"),
      sourcefile: "browser-entry.ts",
    },
    bundle: true,
    format: "iife",
    platform: "browser",
    write: false,
  });
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent("<main id='app'></main>");
    await page.addScriptTag({ content: source.outputFiles[0]!.text });
    const result = await page.evaluate(async () => {
      const api = (window as unknown as {
        GoldenRuntimeTest: {
          RuntimeFrameStager: new (store: unknown) => {
            stageMessage(message: unknown, receivedAt?: number): void;
            metrics: { commits: number; maximumInputToPaintMs: number; previewReplacements: number };
          };
          RuntimeUiStore: new () => {
            frameRevision: number;
            previews: Map<string, unknown>;
          };
        };
      }).GoldenRuntimeTest;
      const store = new api.RuntimeUiStore();
      const stager = new api.RuntimeFrameStager(store);
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      const receivedAt = performance.now();
      const changes = Array.from({ length: 2_000 }, (_, sequence) => ({
        key: { scope: "runtime", entity: "node", field: "value" },
        value: { kind: "integer", value: sequence },
      }));
      stager.stageMessage(
        {
          plane: "observation",
          payload: { kind: "preview", payload: { sequence: 1_999, changes } },
        },
        receivedAt,
      );
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      return {
        commits: stager.metrics.commits,
        latency: stager.metrics.maximumInputToPaintMs,
        replacements: stager.metrics.previewReplacements,
        frameRevision: store.frameRevision,
        previewCount: store.previews.size,
      };
    });
    assert.equal(result.commits, 1);
    assert.equal(result.frameRevision, 1);
    assert.equal(result.previewCount, 1);
    assert.equal(result.replacements, 1_999);
    assert.ok(result.latency < 100, `input-to-paint latency was ${result.latency}ms`);
  } finally {
    await browser.close();
  }
});
