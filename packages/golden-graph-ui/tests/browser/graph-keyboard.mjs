import assert from "node:assert/strict";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { appendFileSync, writeFileSync } from "node:fs";
import { build, preview } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { chromium } from "playwright-core";
import { qualifyKeyboardSockets } from "./keyboard-sockets.mjs";

const root = fileURLToPath(new URL("./fixture/", import.meta.url));
const generated = fileURLToPath(
  new URL("../../../../target/qualification/", import.meta.url),
);
const outDir = resolve(generated, "p11-graph-keyboard");
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
  `p11-graph-keyboard-${new Date().toISOString().replaceAll(/[:.]/g, "-")}.jsonl`,
);
writeFileSync(report, "");
const emit = (value) => {
  appendFileSync(report, JSON.stringify(value) + "\n");
  console.log(JSON.stringify(value));
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
  const inspect = () =>
    page.evaluate(() => window.goldenGraphFixture.keyboardInspect());
  const left = (id) =>
    page
      .locator(`article.node[data-node-id="${id}"]`)
      .evaluate((node) => parseFloat(node.style.left));
  await page.evaluate(() => window.goldenGraphFixture.setupKeyboard());
  assert.equal(
    await page.evaluate(() =>
      window.goldenGraphFixture.keyboardMove("ArrowRight"),
    ),
    true,
  );
  assert.equal(await left("a"), 1.25);
  assert.equal(await left("b"), 24.25);
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardMove("ArrowUp", true),
  );
  assert.equal(
    (await inspect()).positions.find((node) => node.id === "a").y,
    0,
  );
  await page.evaluate(() => window.goldenGraphFixture.keyboardSelect("a"));
  assert.deepEqual((await inspect()).selected, ["a"]);
  await page.evaluate(() => window.goldenGraphFixture.keyboardMode("reject"));
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardMove("ArrowRight"),
  );
  assert.equal(await left("a"), 1.25);
  emit({ kind: "movement", passed: true });

  assert.equal(
    await page.evaluate(() =>
      window.goldenGraphFixture.keyboardSocket("a", "out", "output"),
    ),
    true,
  );
  assert.equal(
    await page
      .locator('[data-socket-direction="output"][data-node-id="a"]')
      .getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(
    await page.evaluate(() =>
      window.goldenGraphFixture.keyboardSocket("c", "no", "input"),
    ),
    false,
  );
  assert.equal(
    await page.evaluate(() =>
      window.goldenGraphFixture.keyboardSocket("b", "in", "input"),
    ),
    true,
  );
  assert.deepEqual((await inspect()).connections, [
    {
      from: { nodeId: "a", socketId: "out" },
      to: { nodeId: "b", socketId: "in" },
    },
  ]);
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardSocket("b", "in", "input"),
  );
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardSocket("a", "out", "output"),
  );
  assert.equal((await inspect()).connections.length, 2);
  await page.evaluate(() => window.goldenGraphFixture.keyboardMode("blocked"));
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardSocket("a", "out", "output"),
  );
  assert.equal(
    await page.evaluate(() =>
      window.goldenGraphFixture.keyboardSocket("b", "in", "input"),
    ),
    false,
  );
  assert.equal((await inspect()).connections.length, 2);
  assert.equal(
    await page.evaluate(() => window.goldenGraphFixture.keyboardCancel()),
    true,
  );
  emit({ kind: "connections", passed: true });

  await page.evaluate(() => window.goldenGraphFixture.keyboardMode("normal"));
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardSocket("a", "out", "output"),
  );
  await page.evaluate(() => window.goldenGraphFixture.keyboardPublication());
  assert.equal((await inspect()).state.endpoint.nodeId, "a");
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardMove("ArrowRight"),
  );
  assert.equal((await inspect()).state.endpoint.nodeId, "a");
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardPublication(true),
  );
  assert.equal((await inspect()).state.endpoint, null);
  emit({ kind: "activity-and-topology", passed: true });

  await page.evaluate(() => window.goldenGraphFixture.keyboardMode("readonly"));
  assert.equal(
    await page.evaluate(() =>
      window.goldenGraphFixture.keyboardMove("ArrowRight"),
    ),
    false,
  );
  assert.equal(
    await page.evaluate(() =>
      window.goldenGraphFixture.keyboardSocket("a", "out", "output"),
    ),
    false,
  );
  await page.evaluate(() => window.goldenGraphFixture.setupKeyboard());
  await page.evaluate(() => window.goldenGraphFixture.keyboardMode("hold"));
  await page.evaluate(() => window.goldenGraphFixture.repeatMovePreview());
  await page.evaluate(() => window.goldenGraphFixture.repeatMovePreview());
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldMove(0));
  assert.equal(await left("a"), 5);
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldMove(1));
  assert.equal(await left("a"), 1);
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardSocket("a", "out", "output"),
  );
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardMove("ArrowRight"),
  );
  await page.evaluate(() => window.goldenGraphFixture.replaceKeyboardScope());
  assert.equal((await inspect()).state.endpoint, null);
  assert.equal(await left("a"), 11);
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardMove("ArrowRight"),
  );
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldMove(2));
  assert.equal(await left("a"), 11.25);
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldMove(3));
  assert.equal(await left("a"), 11);
  assert.deepEqual(errors, []);
  emit({ kind: "lifecycle", passed: true });
  await page.evaluate(() => window.goldenGraphFixture.setupKeyboard());
  const connect = () =>
    page.evaluate(async () => {
      await window.goldenGraphFixture.keyboardSocket("a", "out", "output");
      return window.goldenGraphFixture.keyboardSocket("b", "in", "input");
    });
  await page.evaluate(() =>
    window.goldenGraphFixture.connectionFailureMode("reject"),
  );
  assert.equal(await connect(), true);
  assert.equal((await inspect()).state.message, "Connection request failed.");
  await page.evaluate(() =>
    window.goldenGraphFixture.connectionFailureMode("hold"),
  );
  await connect();
  await connect();
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldConnection(0));
  assert.equal((await inspect()).state.message, "Connection requested.");
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldConnection(1));
  assert.equal((await inspect()).state.message, "Connection request failed.");
  await connect();
  await page.evaluate(() => window.goldenGraphFixture.replaceKeyboardScope());
  await connect();
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldConnection(2));
  assert.equal((await inspect()).state.message, "Connection requested.");
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldConnection(3));
  assert.equal((await inspect()).state.message, "Connection request failed.");
  await connect();
  await page.evaluate(() =>
    window.goldenGraphFixture.keyboardPublication(true),
  );
  await page.evaluate(() => window.goldenGraphFixture.rejectHeldConnection(4));
  assert.equal((await inspect()).state.message, "Connection request failed.");
  assert.deepEqual(errors, []);
  emit({ kind: "asynchronous-connections", passed: true });
  await qualifyKeyboardSockets(page, emit);
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
