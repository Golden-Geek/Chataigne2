import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { verifyCanvasPresentation } from "./canvas-presentation.mjs";
import { verifyCanvasRetention } from "./canvas-retention.mjs";
import { verifyCanvasRouting } from "./canvas-routing.mjs";
import { verifyCanvasOverview } from "./canvas-overview.mjs";

const fixtureRoot = fileURLToPath(new URL("./fixture/", import.meta.url));
const repositoryRoot = fileURLToPath(new URL("../../../../", import.meta.url));
const server = await createServer({
  configFile: false,
  root: fixtureRoot,
  plugins: [svelte({ configFile: false })],
  logLevel: "warn",
  server: {
    host: "127.0.0.1",
    port: 0,
    strictPort: true,
    fs: { allow: [repositoryRoot] },
  },
});

let browser;
try {
  await server.listen();
  const address = server.httpServer?.address();
  assert(address && typeof address === "object");
  const executablePath = process.env.GOLDEN_GRAPH_BROWSER_PATH;
  const channel =
    process.env.GOLDEN_GRAPH_BROWSER_CHANNEL ??
    (process.platform === "win32" ? "msedge" : undefined);
  browser = await chromium.launch({
    headless: true,
    ...(executablePath ? { executablePath } : { channel }),
  });
  const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${address.port}`, {
    waitUntil: "networkidle",
  });
  await page.waitForFunction(() => Boolean(window.goldenGraphFixture));
  const node = (id, x) => ({
    id,
    label: id,
    position: { x, y: 0 },
    inputs: [],
    outputs: [],
  });
  const setNodes = (nodes) =>
    page.evaluate((value) => window.goldenGraphFixture.setNodes(value), nodes);
  const expectVisible = async (total, ids) => {
    await page.waitForFunction(
      ({ total, visible }) => {
        const canvas = document.querySelector(".graph-canvas");
        return (
          canvas?.getAttribute("data-node-count") === String(total) &&
          canvas.getAttribute("data-visible-node-count") === String(visible)
        );
      },
      { total, visible: ids.length },
    );
    assert.deepEqual(
      await page
        .locator(".graph-canvas article.node[data-node-id]")
        .evaluateAll((elements) =>
          elements.map((element) => element.getAttribute("data-node-id")),
        ),
      ids,
    );
  };

  // Data updates exercise the mounted component without desktop input synthesis.
  await setNodes([node("near", 0), node("far", 20)]);
  await expectVisible(2, ["near"]);
  await setNodes([node("near", 0), node("far", 5)]);
  await expectVisible(2, ["near", "far"]);
  await setNodes([node("enormous-coordinate", 1e300)]);
  await expectVisible(1, []);
  await setNodes([]);
  await expectVisible(0, []);
  const retention = await verifyCanvasRetention(page);
  const routing = await verifyCanvasRouting(page);
  await verifyCanvasOverview(page);
  await verifyCanvasPresentation(page);
  assert.deepEqual(errors, []);
  console.log(
    JSON.stringify({
      test: "mounted canvas retained previews, scoped routing, presentation and navigation",
      passed: true,
      browser: await browser.version(),
      retention,
      routing,
    }),
  );
} finally {
  await browser?.close();
  await server.close();
}
