import assert from "node:assert/strict";

/** Verify rendering detail changes without dropping identities or editing authored flags. */
export async function verifyCanvasOverview(page) {
  const nodes = Array.from({ length: 5 }, (_, index) => ({
    id: `overview-${index}`,
    label: `Overview ${index}`,
    position: { x: index * 16, y: 0 },
    size: { width: 13, height: 5 },
    inputs: [{ id: "in", label: "Input" }],
    outputs: [{ id: "out", label: "Output" }],
  }));
  const edges = nodes.slice(1).map((node, index) => ({
    id: `overview-edge-${index}`,
    from: { nodeId: nodes[index].id, socketId: "out" },
    to: { nodeId: node.id, socketId: "in" },
  }));
  await page.evaluate(() => window.goldenGraphFixture.setCanvasSize(100, 60));
  const scene = (nodes, zoom) =>
    page.evaluate(
      ({ nodes, edges, zoom }) =>
        window.goldenGraphFixture.setScene(nodes, edges, {
          selectedNodeIds: ["overview-1"],
          initialCamera: { x: 0, y: 0, zoom },
        }),
      { nodes, edges, zoom },
    );
  await scene(nodes, 0.2);
  await page.waitForFunction(
    () =>
      document.querySelectorAll('article[data-presentation="overview"]')
        .length === 4,
  );
  assert.equal(await page.locator("article.node").count(), 5);
  assert.equal(await page.locator(".wires path.edge").count(), 4);
  assert.equal(
    await page.locator("article.node.selected .socket.input").count(),
    1,
  );
  assert.equal(await page.locator("[data-fixture-node]").count(), 1);
  const updated = nodes.map((node, index) =>
    index === 2
      ? { ...node, label: "Updated overview", active: true, warning: "Warning" }
      : node,
  );
  await scene(updated, 0.2);
  await page.waitForFunction(
    () =>
      document.querySelector(
        'article[data-node-id="overview-2"] .node-title-label',
      )?.textContent === "Updated overview",
  );
  const overview = page.locator('article[data-node-id="overview-2"]');
  assert.equal(
    await overview.locator(".node-title-label").textContent(),
    "Updated overview",
  );
  assert.equal(
    await overview.evaluate((element) => element.classList.contains("active")),
    true,
  );
  assert.equal(await overview.getAttribute("title"), "Warning");
  assert.equal(
    await overview.evaluate((element) => element.style.left),
    "32rem",
  );
  await scene(updated, 1);
  await page.waitForFunction(
    () => document.querySelectorAll("[data-fixture-node]").length === 5,
  );
  assert.equal(await page.locator("article.node").count(), 5);
  assert.equal(
    await page.locator('article[data-presentation="overview"]').count(),
    0,
  );
  assert.equal(await page.locator(".wires path.edge").count(), 4);
  assert.equal(await page.locator("article.node.collapsed").count(), 0);
}
