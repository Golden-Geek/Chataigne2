import assert from "node:assert/strict";

/** Exercise component data and public methods without synthesizing input events. */
export async function verifyCanvasPresentation(page) {
  const setScene = (nodes, edges = [], options = {}) =>
    page.evaluate(
      ({ nodes, edges, options }) =>
        window.goldenGraphFixture.setScene(nodes, edges, options),
      { nodes, edges, options },
    );
  const node = {
    id: "rendered",
    label: "Original label",
    position: { x: 0, y: 0 },
    inputs: [{ id: "in", label: "Input", defaultParamId: "default" }],
    outputs: [{ id: "out", label: "Output" }],
    bypassConnections: [{ inputSocketId: "in", outputSocketId: "out" }],
  };
  const article = page.locator('article.node[data-node-id="rendered"]');
  await setScene([node], [], {
    selectedNodeIds: [node.id],
    socketLabels: "never",
  });
  await page.waitForFunction(() =>
    document
      .querySelector('article.node[data-node-id="rendered"]')
      ?.classList.contains("selected"),
  );
  assert.equal(
    await article.locator("[data-fixture-node]").textContent(),
    node.label,
  );
  assert.equal(
    await article.locator("[data-fixture-header]").textContent(),
    node.label,
  );
  assert.equal(
    await article.locator("[data-fixture-input]").textContent(),
    "rendered:in",
  );
  assert.equal(
    await article.locator("[data-fixture-output]").textContent(),
    "rendered:out",
  );
  assert.equal(await page.locator("[data-fixture-toolbar]").count(), 1);
  assert.equal(await page.locator("[data-fixture-world]").count(), 1);
  const labelOpacity = () =>
    article
      .locator(".socket.input span:not(.pin)")
      .evaluate((element) => getComputedStyle(element).opacity);
  await page.waitForFunction(() => {
    const label = document.querySelector(
      "article.node .socket.input span:not(.pin)",
    );
    return label && getComputedStyle(label).opacity === "0";
  });
  assert.equal(await labelOpacity(), "0");

  const loop = {
    id: "loop",
    from: { nodeId: node.id, socketId: "out" },
    to: { nodeId: node.id, socketId: "in" },
    color: "#f00",
    targetColor: "#00f",
    active: true,
  };
  await setScene(
    [{ ...node, label: "Updated label", active: true, enabled: false }],
    [loop],
    {
      selectedEdgeIds: [loop.id],
      socketLabels: "always",
      routeEdgesAroundNodes: true,
    },
  );
  await page.waitForFunction(
    () =>
      document.querySelector("[data-fixture-node]")?.textContent ===
      "Updated label",
  );
  assert.equal(
    await article.evaluate((element) => element.classList.contains("selected")),
    false,
  );
  assert.equal(
    await article.evaluate(
      (element) =>
        element.classList.contains("active") &&
        element.classList.contains("disabled"),
    ),
    true,
  );
  await page.waitForFunction(() => {
    const label = document.querySelector(
      "article.node .socket.input span:not(.pin)",
    );
    return label && getComputedStyle(label).opacity === "1";
  });
  assert.equal(await labelOpacity(), "1");
  assert.equal(await article.locator("[data-fixture-input]").count(), 0);
  assert.equal(await article.locator(".socket.input.connected").count(), 1);
  assert.equal(await article.locator(".node-bypass-wires path").count(), 1);
  assert.equal(
    await page.locator(".wires path.edge.selected.active.gradient").count(),
    1,
  );
  assert.equal(await page.locator(".wires linearGradient stop").count(), 2);
  assert(await page.locator(".wires path.edge").getAttribute("d"));

  await setScene([{ ...node, collapsed: true }], [loop]);
  await page.waitForFunction(() =>
    document.querySelector("article.node")?.classList.contains("collapsed"),
  );
  assert.equal(await article.locator(".collapsed-socket").count(), 2);
  assert.equal(
    await article.locator("[data-fixture-node], [data-fixture-header]").count(),
    0,
  );

  const nestedInput = {
    ...node.inputs[0],
    children: [{ id: "component", label: "Component", parentId: "in" }],
  };
  await setScene(
    [{ ...node, inputs: [nestedInput] }],
    [{ ...loop, to: { ...loop.to, socketId: "component" } }],
  );
  await page.waitForFunction(() =>
    document.querySelector('.socket.input[data-socket-id="component"]'),
  );
  assert.equal(
    await article
      .locator('.socket.input[data-socket-id="component"].connected')
      .count(),
    1,
  );
  assert.equal(await article.locator("[data-fixture-input]").count(), 0);

  await setScene([{ ...node, socketPlacement: "header" }], [loop]);
  await page.waitForFunction(() =>
    document.querySelector(".header-socket.input:not(.collapsed-socket)"),
  );
  assert.equal(await article.locator(".header-socket").count(), 2);

  const positionedNode = (id, x) => ({
    id,
    label: id,
    position: { x, y: 0 },
    inputs: [],
    outputs: [],
  });
  await setScene(
    [positionedNode("near", 0), positionedNode("selected", 30)],
    [],
    {
      selectedNodeIds: ["selected"],
      initialCamera: { x: 0, y: 0, zoom: 1 },
    },
  );
  await page.waitForFunction(() =>
    document.querySelector('article.node[data-node-id="near"]'),
  );
  assert.equal(
    await page.evaluate(() => window.goldenGraphFixture.frameSelection()),
    true,
  );
  await page.waitForFunction(() => {
    const center = window.goldenGraphFixture.coordinates(0, 0).center;
    return center && Math.abs(center.x - 36.5) < 0.001;
  });
  assert.equal(
    await page
      .locator('article.node[data-node-id="selected"].selected')
      .count(),
    1,
  );
  const probe = await page.evaluate(() => {
    const bounds = document
      .querySelector(".graph-canvas")
      .getBoundingClientRect();
    return window.goldenGraphFixture.coordinates(
      bounds.left + bounds.width / 2,
      bounds.top + bounds.height / 2,
    );
  });
  assert.deepEqual(probe.world, probe.center);
  assert.equal(
    await page.evaluate(() => window.goldenGraphFixture.home()),
    true,
  );
  await page.waitForFunction(
    () => document.querySelectorAll("article.node[data-node-id]").length === 2,
  );
}
