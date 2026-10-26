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
const outDir = resolve(generated, "p11-graph-keyboard-qualification");
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
  `p11-graph-keyboard-qualification-${new Date().toISOString().replaceAll(/[:.]/g, "-")}.jsonl`,
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
  for (const count of [1000, 10000, 20000, 50000, 100000]) {
    emit({
      kind: "setup",
      ...(await page.evaluate(
        (count) => window.goldenGraphFixture.setupKeyboardQualification(count),
        count,
      )),
    });
    for (const name of [
      "single-move",
      "multi-move",
      "output-first",
      "input-first",
      "compound",
      "collapsed",
    ]) {
      const result = await page.evaluate(
        (name) => window.goldenGraphFixture.runKeyboardQualification(name),
        name,
      );
      emit({ kind: "timing", ...result });
      assert.equal(result.passed, true, `Keyboard ${name} failed at ${count}`);
    }
  }
  assert.deepEqual(errors, []);
  emit({ kind: "result", passed: true, errors });
} catch (error) {
  emit({ kind: "failure", message: String(error), errors });
  throw error;
} finally {
  await browser?.close();
  await new Promise((done, reject) =>
    server.httpServer.close((error) => (error ? reject(error) : done())),
  );
}
