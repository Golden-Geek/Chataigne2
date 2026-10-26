import assert from "node:assert/strict";

/** Calls production operations and inspects native controls; no input is synthesized. */
export async function qualifyKeyboardSockets(page, emit) {
  const call = (method, ...args) =>
    page.evaluate(
      ({ method, args }) => window.goldenGraphFixture[method](...args),
      { method, args },
    );
  const port = (node, socket, direction) =>
    page.locator(
      `article[data-node-id="${node}"] button[data-socket-id="${socket}"][data-socket-direction="${direction}"]`,
    );
  const inspect = () => call("keyboardInspect");
  const connect = (node, socket, direction) =>
    call("keyboardSocket", node, socket, direction);
  const expand = (node, socket, direction = "input") =>
    call("keyboardExpand", node, socket, direction);
  const collapsed = (node) =>
    page
      .locator(`article[data-node-id="${node}"]`)
      .evaluate((element) => element.classList.contains("collapsed"));
  await call("setupSockets", "body");
  assert.equal(await port("a", "disabled", "output").isDisabled(), true);
  assert.equal(await connect("a", "disabled", "output"), false);
  assert.equal(await connect("a", "leaf", "output"), false);
  assert.equal(await expand("a", "vector", "output"), true);
  assert.equal(await connect("a", "leaf", "output"), false);
  assert.equal(await expand("a", "x", "output"), true);
  assert.equal(await connect("a", "leaf", "output"), true);
  assert.equal(
    await port("a", "leaf", "output").getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(await connect("b", "header", "input"), true);
  assert.deepEqual((await inspect()).connections.at(-1), {
    from: { nodeId: "a", socketId: "leaf" },
    to: { nodeId: "b", socketId: "header" },
  });
  assert.equal(await expand("b", "vector"), true);
  assert.equal(await expand("b", "x"), true);
  assert.equal(await connect("b", "leaf", "input"), true);
  assert.equal(await connect("a", "out", "output"), true);
  assert.deepEqual((await inspect()).connections.at(-1), {
    from: { nodeId: "a", socketId: "out" },
    to: { nodeId: "b", socketId: "leaf" },
  });
  await connect("b", "y", "input");
  await expand("b", "vector");
  assert.equal((await inspect()).state.endpoint, null);
  emit({ kind: "body-header-and-nested-sockets", passed: true });

  await call("setupSockets", "header");
  assert.equal(await port("a", "disabled", "output").isDisabled(), true);
  assert.equal(await connect("a", "disabled", "output"), false);
  await connect("b", "in", "input");
  await connect("a", "out", "output");
  assert.deepEqual((await inspect()).connections.at(-1), {
    from: { nodeId: "a", socketId: "out" },
    to: { nodeId: "b", socketId: "in" },
  });
  await call("setupSockets", "collapsed");
  assert.equal(await connect("a", "vector", "output"), false);
  await connect("a", "out", "output");
  await connect("b", "in", "input");
  assert.equal((await inspect()).connections.length, 1);
  const collapseButton = page.locator(
    'article[data-node-id="a"] .node-collapse',
  );
  assert.equal(await collapseButton.getAttribute("aria-expanded"), "false");
  await collapseButton.evaluate((element) => element.focus());
  assert.equal(await call("keyboardCollapse", "a"), true);
  assert.equal(await collapseButton.getAttribute("aria-expanded"), "true");
  assert.equal(
    await collapseButton.evaluate(
      (element) => document.activeElement === element,
    ),
    true,
  );
  assert.equal(await connect("a", "vector", "output"), true);
  await connect("b", "in", "input");
  assert.equal(await call("keyboardCollapse", "b"), true);
  await connect("b", "vector", "input");
  await call("setCollapseMode", "reject");
  await call("keyboardCollapse", "b");
  assert.equal(await collapsed("b"), false);
  assert.equal((await inspect()).state.message, "Node view change failed.");
  await call("keyboardMode", "readonly");
  assert.equal(await call("keyboardCollapse", "a"), false);
  assert.equal(await call("keyboardMove", "ArrowRight"), false);
  assert.equal(await connect("a", "out", "output"), false);
  assert.equal(await page.locator(".node-collapse").count(), 0);
  emit({ kind: "header-collapsed-and-readonly", passed: true });

  await call("setupSockets", "body");
  await call("setSocketConnection", "vector");
  await expand("b", "vector");
  await expand("b", "x");
  assert.equal(await port("b", "x", "input").isDisabled(), true);
  assert.equal(await port("b", "leaf", "input").isDisabled(), true);
  assert.equal(await connect("b", "leaf", "input"), false);
  await call("setSocketConnection", "leaf");
  assert.equal(
    await port("b", "x", "output").count(),
    0,
    "An input connection expanded output sockets with the same IDs",
  );
  assert.equal(await port("b", "vector", "input").isDisabled(), true);
  assert.equal(await port("b", "x", "input").isDisabled(), true);
  assert.equal(await port("b", "leaf", "input").isDisabled(), false);
  assert.equal(await expand("b", "vector"), false);
  assert.equal(await expand("b", "x"), false);
  assert.equal(
    await page
      .locator(
        'article[data-node-id="b"] .socket-expander[aria-expanded="true"][disabled]',
      )
      .count(),
    2,
  );
  await connect("b", "leaf", "input");
  await connect("a", "out", "output");
  await call("setSocketConnection", null);
  await call("keyboardPublication");
  assert.equal(await port("b", "leaf", "input").count(), 1);
  await call("replaceKeyboardScope");
  assert.equal(await port("b", "x", "input").count(), 0);
  assert.equal(await port("b", "leaf", "input").count(), 0);
  emit({ kind: "compound-exclusion-and-scope", passed: true });

  await call("setupSockets", "collapsed");
  await call("setCollapseMode", "hold");
  await call("keyboardCollapse", "a");
  await call("keyboardCollapse", "a");
  await call("keyboardCollapse", "a");
  await call("rejectHeldCollapse", 0);
  assert.equal(await collapsed("a"), false);
  await call("rejectHeldCollapse", 2);
  assert.equal(await collapsed("a"), true);
  assert.equal((await inspect()).state.message, "Node view change failed.");
  await call("setupSockets", "collapsed");
  await call("setCollapseMode", "hold");
  await call("keyboardCollapse", "a");
  await call("replaceKeyboardScope");
  await call("keyboardCollapse", "a");
  await call("rejectHeldCollapse", 0);
  assert.equal(await collapsed("a"), false);
  await call("rejectHeldCollapse", 1);
  assert.equal(await collapsed("a"), true);
  await call("setupSockets", "body");
  await call("keyboardMode", "reject");
  await call("keyboardMove", "ArrowRight");
  assert.equal((await inspect()).state.message, "Move request failed.");
  await call("keyboardMode", "hold");
  await call("keyboardMove", "ArrowRight");
  await connect("a", "out", "output");
  await call("rejectHeldMove", 0);
  assert.equal(
    (await inspect()).state.message,
    "Choose an input socket to connect.",
  );
  emit({ kind: "collapse-and-move-failure-lifetimes", passed: true });
}
