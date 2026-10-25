import assert from "node:assert/strict";
import {
  writeFileSync,
  appendFileSync,
  readFileSync,
  readdirSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { cpus, platform, release } from "node:os";
import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { build, preview } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const fixtureRoot = fileURLToPath(new URL("./fixture/", import.meta.url));
const repositoryRoot = fileURLToPath(new URL("../../../../", import.meta.url));
const generatedRoot = resolve(repositoryRoot, "target/qualification");
const outDir = resolve(generatedRoot, "p10-canvas");
assert.equal(relative(generatedRoot, outDir), "p10-canvas");
await build({
  configFile: false,
  root: fixtureRoot,
  plugins: [svelte({ configFile: false })],
  logLevel: "warn",
  build: { outDir, emptyOutDir: true },
});
const server = await preview({
  configFile: false,
  root: fixtureRoot,
  logLevel: "warn",
  build: { outDir },
  preview: { host: "127.0.0.1", port: 0, strictPort: true },
});
const runId = new Date().toISOString().replaceAll(/[:.]/g, "-");
const report = resolve(generatedRoot, `p10-canvas-${runId}.jsonl`);
writeFileSync(report, "");
const emit = (value) => {
  appendFileSync(report, JSON.stringify(value) + "\n");
  const { warmups, samples, ...summary } = value;
  console.log(JSON.stringify(summary));
};
const files = (directory) =>
  readdirSync(directory, { withFileTypes: true }).flatMap((entry) =>
    entry.name === "node_modules"
      ? []
      : entry.isDirectory()
        ? files(resolve(directory, entry.name))
        : [resolve(directory, entry.name)],
  );
const hash = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
const source = files(resolve(repositoryRoot, "packages/golden-graph-ui"))
  .filter((path) => /\.(ts|svelte|mjs|json)$/.test(path))
  .sort()
  .map((path) => ({
    path: relative(repositoryRoot, path),
    sha256: hash(path),
  }));
emit({
  kind: "source",
  files: source,
  bundle: files(outDir)
    .sort()
    .map((path) => ({ path: relative(outDir, path), sha256: hash(path) })),
});
console.log(JSON.stringify({ kind: "report", path: report }));
const actions = [
  "selection",
  "drag-single",
  "drag-multi",
  "pan",
  "zoom",
  "frame-selection",
  "home",
  "activity",
  "payload",
  "loop",
  "obstacle",
];
let browser;
let passed = true;
try {
  const address = server.httpServer.address();
  assert(address && typeof address === "object");
  const executablePath = process.env.GOLDEN_GRAPH_BROWSER_PATH;
  const channel =
    process.env.GOLDEN_GRAPH_BROWSER_CHANNEL ??
    (process.platform === "win32" ? "msedge" : undefined);
  browser = await chromium.launch({
    headless: true,
    ...(executablePath ? { executablePath } : { channel }),
  });
  const page = await browser.newPage({
    viewport: { width: 1680, height: 1200 },
  });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  emit({
    kind: "environment",
    at: new Date().toISOString(),
    browser: await browser.version(),
    os: `${platform()} ${release()}`,
    cpu: cpus()[0]?.model,
    logicalProcessors: cpus().length,
    viewport: { width: 1680, height: 1200 },
    production: true,
  });
  await page.goto(`http://127.0.0.1:${address.port}`, {
    waitUntil: "networkidle",
  });
  await page.waitForFunction(() => Boolean(window.goldenGraphFixture));
  for (const count of [1000, 10000, 20000, 50000, 100000]) {
    emit({
      kind: "setup",
      ...(await page.evaluate(
        (count) => window.goldenGraphFixture.setupQualification(count),
        count,
      )),
    });
    for (const action of actions) {
      try {
        const result = await page.evaluate(
          (action) => window.goldenGraphFixture.runQualificationAction(action),
          action,
        );
        passed &&= result.passed;
        emit({ kind: "action", ...result });
      } catch (error) {
        passed = false;
        emit({ kind: "failure", nodes: count, action, error: String(error) });
      }
    }
  }
  for (const varied of [false, true]) {
    try {
      const result = await page.evaluate(
        (varied) => window.goldenGraphFixture.runDenseQualification(varied),
        varied,
      );
      passed &&= result.passed;
      emit({ kind: "dense", ...result });
    } catch (error) {
      passed = false;
      emit({
        kind: "failure",
        scene: varied ? "varied" : "parallel",
        error: String(error),
      });
    }
  }
  if (errors.length) passed = false;
  emit({ kind: "result", passed, errors });
  process.exitCode = passed ? 0 : 1;
} finally {
  await browser?.close();
  await new Promise((resolve, reject) =>
    server.httpServer.close((error) => (error ? reject(error) : resolve())),
  );
}
