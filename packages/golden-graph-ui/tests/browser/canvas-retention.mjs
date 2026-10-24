import assert from "node:assert/strict";

export async function verifyCanvasRetention(page) {
  const measurements = [];
  for (const nodes of [1_000, 10_000, 20_000, 50_000, 100_000]) {
    await page.evaluate(
      (count) => window.goldenGraphFixture.setScaleScene(count),
      nodes,
    );
    await page.waitForFunction(
      (count) =>
        document
          .querySelector(".graph-canvas")
          ?.getAttribute("data-node-count") === String(count),
      nodes,
    );
    await page.evaluate(() => window.goldenGraphFixture.settleRouting());
    assert.equal(
      await page.locator('article.node[data-node-id="n0"]').count(),
      1,
    );
    const visibleNodes = await page.locator("article.node").count();
    const visibleEdges = await page.locator(".wires path.edge").count();
    assert.equal(visibleNodes, 1);
    // Path-bounds culling also retains the first row-wrap wire with offscreen endpoints.
    assert.equal(visibleEdges, 2);
    const samples = await page.evaluate(() =>
      window.goldenGraphFixture.samplePreviews(3, 31),
    );
    for (const sample of samples) {
      assert.deepEqual(sample.work, {
        sourceNodes: 0,
        projectedNodes: 1,
        measuredNodes: 1,
        spatialUpdates: 1,
        materializedNodes: 0,
        resets: 0,
      });
    }
    const probes = await page.evaluate(() =>
      window.goldenGraphFixture.probeActivityAndPreview(),
    );
    assert.equal(probes.activity.work.sourceNodes, 1);
    assert.equal(probes.activity.work.measuredNodes, 0);
    assert.equal(probes.activity.work.spatialUpdates, 0);
    assert.equal(probes.activity.work.materializedNodes, 0);
    assert.deepEqual(probes.activity.after, {
      ...probes.activity.before,
      sequence: probes.activity.before.sequence + 1,
      activity: probes.activity.before.activity + 1,
    });
    assert.equal(probes.preview.work.sourceNodes, 0);
    assert.equal(probes.preview.work.measuredNodes, 0);
    assert.equal(probes.preview.work.spatialUpdates, 0);
    assert.equal(probes.preview.work.materializedNodes, 0);
    assert.deepEqual(probes.preview.revision, {
      ...probes.activity.after,
      sequence: probes.activity.after.sequence + 1,
      preview: probes.activity.after.preview + 1,
    });
    assert.equal(probes.resize.measuredNodes, 1);
    assert.equal(probes.resize.spatialUpdates, 1);
    assert.equal(probes.resize.materializedNodes, 0);
    assert.equal(probes.finish.after.pendingRoutes, 0);
    assert.equal(
      probes.finish.after.routeJobs - probes.finish.before.routeJobs,
      1,
    );
    assert.equal(probes.finish.projection.materializedNodes, 0);
    measurements.push({
      nodes,
      edges: nodes - 1,
      visibleNodes,
      visibleEdges,
      samples,
      probes,
    });
  }
  return measurements;
}
