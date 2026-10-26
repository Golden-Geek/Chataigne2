import assert from "node:assert/strict";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { appendFileSync, writeFileSync } from "node:fs";
import { build, preview } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { chromium } from "playwright-core";

const root = fileURLToPath(new URL("./fixture/", import.meta.url));
const generated = fileURLToPath(
  new URL("../../../../target/qualification/", import.meta.url),
);
const outDir = resolve(generated, "p11-graph-discovery");
await build({
  configFile: false,
  root,
  plugins: [svelte({ configFile: false })],
  logLevel: "warn",
  build: { outDir, emptyOutDir: true },
});
const server = await preview({
  configFile: false,
  root,
  logLevel: "warn",
  build: { outDir },
  preview: { host: "127.0.0.1", port: 0, strictPort: true },
});
const report = resolve(
  generated,
  `p11-graph-discovery-${new Date().toISOString().replaceAll(/[:.]/g, "-")}.jsonl`,
);
writeFileSync(report, "");
const emit = (value) => {
  appendFileSync(report, JSON.stringify(value) + "\n");
  console.log(
    JSON.stringify(
      value.kind === "timing"
        ? { ...value, samples: value.samples.length }
        : value,
    ),
  );
};
let browser;
const errors = [];
try {
  browser = await chromium.launch({
    headless: true,
    ...(process.env.GOLDEN_GRAPH_BROWSER_PATH
      ? { executablePath: process.env.GOLDEN_GRAPH_BROWSER_PATH }
      : { channel: "msedge" }),
  });
  const page = await browser.newPage({
    viewport: { width: 1680, height: 1200 },
  });
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}`, {
    waitUntil: "networkidle",
  });
  emit({
    kind: "environment",
    production: true,
    browser: await browser.version(),
    report,
  });
  const tiers = process.env.GOLDEN_GRAPH_DISCOVERY_TIERS
    ? process.env.GOLDEN_GRAPH_DISCOVERY_TIERS.split(",").map(Number)
    : [1000, 10000, 20000, 50000, 100000];
  for (const count of tiers) {
    const setup = await page.evaluate(
      (size) => window.goldenGraphFixture.setupDiscovery(size),
      count,
    );
    emit({ kind: "setup", ...setup });
    for (const action of ["search", "clear", "page", "reveal"]) {
      const result = await page.evaluate(
        (name) => window.goldenGraphFixture.runDiscoveryAction(name),
        action,
      );
      emit({ kind: "timing", ...result });
      assert.equal(
        result.passed,
        true,
        `Discovery ${action} failed at ${count}`,
      );
    }
    emit({
      kind: "correctness",
      ...(await page.evaluate(() =>
        window.goldenGraphFixture.discoveryCorrectness(),
      )),
    });
    // A source-generation replacement retires in-flight search and closes the finder.
    await page.evaluate(async () => {
      await window.goldenGraphFixture.showNodeFinder();
      const pending = window.goldenGraphFixture.searchNodes(
        "unmatched stale query",
      );
      await window.goldenGraphFixture.replaceKeyboardScope();
      await pending;
    });
    assert.equal(
      await page.evaluate(
        () => window.goldenGraphFixture.nodeFinderState().open,
      ),
      false,
    );
    assert.equal(
      await page.evaluate(() =>
        document.activeElement?.classList.contains("graph-canvas"),
      ),
      true,
      "Source replacement lost finder focus",
    );
    emit({ kind: "source-replacement", count, passed: true });
    assert.deepEqual(
      await page.evaluate(() =>
        window.goldenGraphFixture.replaceCanvasWhileSearching(),
      ),
      { staleReveal: false, finderOpen: false },
    );
    emit({ kind: "unmount", count, passed: true });
  }
  assert.deepEqual(errors, []);
  emit({ kind: "result", passed: true, errors, tiers });
} catch (error) {
  emit({ kind: "failure", message: String(error), errors });
  throw error;
} finally {
  await browser?.close();
  await new Promise((done, reject) =>
    server.httpServer.close((error) => (error ? reject(error) : done())),
  );
}
