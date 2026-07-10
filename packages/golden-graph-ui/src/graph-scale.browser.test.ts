import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";

import { build } from "esbuild";
import { chromium } from "playwright";

test("real Chromium keeps a 100,000-node graph query and paint viewport-bounded", async () => {
  const source = await build({
    stdin: {
      contents: `
        import { SpatialIndex } from "./spatial-index.ts";
        window.GoldenGraphScaleTest = { SpatialIndex };
      `,
      resolveDir: path.resolve("packages/golden-graph-ui/src"),
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
    await page.setContent("<main id='viewport'></main>");
    await page.addScriptTag({ content: source.outputFiles[0]!.text });
    const result = await page.evaluate(async () => {
      const api = (window as unknown as {
        GoldenGraphScaleTest: {
          SpatialIndex: new (cellSize: number) => {
            upsert(id: string, rectangle: { x: number; y: number; width: number; height: number }): void;
            query(area: { x: number; y: number; width: number; height: number }): string[];
          };
        };
      }).GoldenGraphScaleTest;
      const index = new api.SpatialIndex(100);
      const buildStarted = performance.now();
      for (let value = 0; value < 100_000; value += 1) {
        index.upsert(`node-${value}`, {
          x: (value % 1_000) * 100,
          y: Math.floor(value / 1_000) * 100,
          width: 20,
          height: 20,
        });
      }
      const buildMs = performance.now() - buildStarted;
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      const inputAt = performance.now();
      const visible = index.query({ x: 49_950, y: 4_950, width: 200, height: 200 });
      const viewport = document.querySelector("#viewport")!;
      viewport.replaceChildren(
        ...visible.map((id) => {
          const node = document.createElement("div");
          node.dataset.nodeId = id;
          return node;
        }),
      );
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      return {
        buildMs,
        inputToPaintMs: performance.now() - inputAt,
        visibleCount: visible.length,
        renderedCount: viewport.childElementCount,
      };
    });
    assert.ok(result.buildMs < 5_000, `100k index build took ${result.buildMs}ms`);
    assert.ok(result.inputToPaintMs < 100, `viewport input-to-paint took ${result.inputToPaintMs}ms`);
    assert.ok(result.visibleCount > 0 && result.visibleCount <= 9);
    assert.equal(result.renderedCount, result.visibleCount);
    process.stderr.write(
      `100k graph qualification: build=${result.buildMs.toFixed(3)}ms input-to-paint=${result.inputToPaintMs.toFixed(3)}ms visible=${result.visibleCount}\n`,
    );
  } finally {
    await browser.close();
  }
});
