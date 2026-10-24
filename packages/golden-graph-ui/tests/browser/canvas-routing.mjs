import assert from "node:assert/strict";

export async function verifyCanvasRouting(page) {
  await page.setViewportSize({ width: 800, height: 1200 });
  await page.evaluate(() => window.goldenGraphFixture.setCanvasSize(10, 64));
  const node = (id, x, y = 0) => ({
    id,
    label: id,
    position: { x, y },
    inputs: [{ id: "in", label: "Input" }],
    outputs: [{ id: "out", label: "Output" }],
  });
  const nodes = [
    node("left", -40),
    node("right", 40),
    node("far-left", -40, 50),
    node("far-right", 40, 50),
  ];
  const edge = (id, from, to) => ({
    id,
    from: { nodeId: from, socketId: "out" },
    to: { nodeId: to, socketId: "in" },
  });
  const edges = [
    edge("crossing", "left", "right"),
    edge("unrelated", "far-left", "far-right"),
  ];
  const setScene = (nodes, edges, routeEdgesAroundNodes) =>
    page.evaluate(
      ({ nodes, edges, routeEdgesAroundNodes }) =>
        window.goldenGraphFixture.setScene(nodes, edges, {
          routeEdgesAroundNodes,
        }),
      { nodes, edges, routeEdgesAroundNodes },
    );
  const settle = () =>
    page.evaluate(() => window.goldenGraphFixture.settleRouting());
  const paths = () =>
    page
      .locator(".wires path.edge")
      .evaluateAll((elements) =>
        elements.map((element) => element.getAttribute("d")),
      );

  await setScene(nodes, edges, false);
  await page.waitForFunction(
    () => document.querySelectorAll(".wires path.edge").length === 2,
  );
  assert.equal(await page.locator("article.node").count(), 0);
  assert.equal(
    await page
      .locator(".wires path.edge")
      .first()
      .evaluate((path) => path.isPointInStroke({ x: 80, y: 2.805 * 16 })),
    true,
  );
  const curves = await paths();

  await setScene(nodes, edges, true);
  const baseline = await settle();
  const routed = await paths();
  assert.equal(routed.length, 2);
  assert(routed.every((path) => !path.includes(" C ")));
  assert.equal(
    await page
      .locator(".wires path.edge")
      .first()
      .evaluate((path) => path.isPointInStroke({ x: 80, y: 2.805 * 16 })),
    true,
  );

  await setScene(nodes, edges, false);
  await page.waitForFunction(() =>
    [...document.querySelectorAll(".wires path.edge")].every((path) =>
      path.getAttribute("d").includes(" C "),
    ),
  );
  await setScene(nodes, edges, true);
  await settle();
  assert.deepEqual(await paths(), routed);

  await setScene([...nodes, node("obstacle", 3, 1)], edges, true);
  const obstructed = await settle();
  const detoured = await paths();
  assert.equal(obstructed.invalidatedRoutes - baseline.invalidatedRoutes, 1);
  assert.equal(
    obstructed.edgeGeometryUpdates - baseline.edgeGeometryUpdates,
    0,
  );
  assert.notEqual(detoured[0], routed[0]);
  assert.equal(detoured[1], routed[1]);
  assert.equal(
    await page
      .locator(".wires path.edge")
      .first()
      .evaluate((path) => path.isPointInStroke({ x: 80, y: 2.805 * 16 })),
    false,
  );

  await setScene([...nodes, node("obstacle", 200, 100)], edges, true);
  const restoredWork = await settle();
  assert.deepEqual(await paths(), routed);
  assert.equal(
    restoredWork.invalidatedRoutes - obstructed.invalidatedRoutes,
    1,
  );

  // A self-loop uses an exterior route even with the node body covering the direct curve.
  await setScene([node("loop", 0)], [edge("loop-edge", "loop", "loop")], true);
  await settle();
  const loop = await page.locator(".wires path.edge").getAttribute("d");
  assert(loop && !loop.includes(" C "));
  assert.equal(
    await page.locator(".wires path.edge").evaluate((path) => {
      const bounds = path.getBBox();
      const node = document.querySelector("article.node");
      return (
        bounds.height > 0 &&
        (bounds.y < 0 ||
          bounds.y + bounds.height > node.getBoundingClientRect().height)
      );
    }),
    true,
  );

  const overflowStarted = performance.now();
  const parallel = Array.from({ length: 1100 }, (_, index) =>
    edge(`parallel-${index}`, "left", "right"),
  );
  await setScene(nodes.slice(0, 2), parallel, true);
  const overflow = await settle();
  const overflowWallMs = performance.now() - overflowStarted;
  assert.equal(await page.locator(".wires path.edge").count(), 1100);
  assert.equal(overflow.cachedRoutes, 1024);
  assert.equal(overflow.pendingRoutes, 0);
  assert(overflow.maxSliceJobs <= 4);
  assert(overflow.maxSliceVisits <= 4096);
  assert(overflow.maxSliceSmoothingChecks <= 2048);
  assert(
    (await page.locator(".wires path.edge").last().getAttribute("d")).includes(
      " C ",
    ),
  );
  const idle = await page.evaluate(async () => {
    const before = window.goldenGraphFixture.routingWork();
    for (let frame = 0; frame < 10; frame += 1)
      await new Promise(requestAnimationFrame);
    return { before, after: window.goldenGraphFixture.routingWork() };
  });
  assert.deepEqual(idle.after, idle.before);

  await setScene([], [], true);
  await settle();
  assert.equal(await page.locator(".wires path.edge").count(), 0);
  await page.evaluate(() => window.goldenGraphFixture.setCanvasSize(10, 10));
  await page.setViewportSize({ width: 800, height: 600 });
  return {
    crossingPaths: curves.length,
    obstacleInvalidations:
      obstructed.invalidatedRoutes - baseline.invalidatedRoutes,
    unrelatedPathPreserved: detoured[1] === routed[1],
    loopExterior: true,
    overflow: {
      edges: 1100,
      wallMs: overflowWallMs,
      work: overflow,
      idleStable: true,
    },
  };
}
