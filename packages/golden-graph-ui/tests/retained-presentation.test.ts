import { describe, expect, it } from "vitest";
import { GraphPresentationAdapter } from "../presentation-document";
import { RetainedGraphPresentation } from "../retained-presentation";
import type { GraphNodePresentationChanges } from "../presentation-projection";
import type { GraphNode, GraphPresentationDocument } from "../types";

const node = (id: string, x = 0, y = 0): GraphNode => ({
  id,
  label: id,
  position: { x, y },
  inputs: [],
  outputs: [],
});
const overlays = (): GraphNodePresentationChanges => ({
  dragPositions: {},
  optimisticPositions: {},
  resizeSizes: {},
  optimisticSizes: {},
  optimisticLabels: {},
  optimisticCollapsed: {},
  optimisticEnabled: {},
});
const synchronize = (
  view: RetainedGraphPresentation,
  document: GraphPresentationDocument,
  changes = overlays(),
) =>
  view.synchronize(
    document,
    changes,
    () => 2,
    () => 2,
    new Set(),
  );
const box = (x: number, y = 0) => ({
  left: x,
  right: x + 2,
  top: y,
  bottom: y + 2,
});

describe("retained canvas presentation", () => {
  it.each([1_000, 10_000, 50_000, 100_000])(
    "keeps one preview proportional at %i authored nodes",
    (count) => {
      const nodes = Array.from({ length: count }, (_, index) =>
        node(String(index), (index % 128) * 20, Math.floor(index / 128) * 20),
      );
      const adapter = new GraphPresentationAdapter();
      const document = adapter.update(nodes, []);
      const view = new RetainedGraphPresentation();
      synchronize(view, document);
      const identities = view.nodesById;
      const baseline = view.statistics;
      const changes = overlays();
      changes.dragPositions["0"] = { x: -20, y: -20 };
      synchronize(view, document, changes);
      expect(view.nodesById).toBe(identities);
      expect(view.nodesById.get("1")).toBe(nodes[1]);
      expect(view.query(box(-20, -20)).map((value) => value.id)).toEqual(["0"]);
      expect(view.query(box(0))).toEqual([]);
      const moved = view.statistics;
      expect(moved.sourceNodes - baseline.sourceNodes).toBe(0);
      expect(moved.projectedNodes - baseline.projectedNodes).toBe(1);
      expect(moved.measuredNodes - baseline.measuredNodes).toBe(1);
      expect(moved.spatialUpdates - baseline.spatialUpdates).toBe(1);
      expect(moved.materializedNodes - baseline.materializedNodes).toBe(0);
      synchronize(view, document);
      expect(view.nodesById.get("0")).toBe(nodes[0]);
      expect(view.query(box(0))).toEqual([nodes[0]]);
      expect(view.statistics.measuredNodes - moved.measuredNodes).toBe(1);
    },
  );

  it("updates activity/payload and preview stamps without moving spatial membership", () => {
    const adapter = new GraphPresentationAdapter();
    const view = new RetainedGraphPresentation();
    const first = adapter.update([node("a"), node("b", 100)], []);
    synchronize(view, first);
    const baseline = view.statistics;
    const revision = view.geometryRevision;
    const active = { ...first.nodes[0], active: true, color: "red" };
    const updated = adapter.update([active, first.nodes[1]], []);
    synchronize(view, updated);
    expect(view.query(box(0))[0]).toBe(active);
    expect(view.geometryRevision).toBe(revision);
    expect(view.statistics.measuredNodes).toBe(baseline.measuredNodes);
    expect(view.statistics.spatialUpdates).toBe(baseline.spatialUpdates);
    const preview = adapter.update(updated.nodes, updated.edges, 1);
    const beforePreview = view.statistics;
    synchronize(view, preview);
    expect(view.statistics).toEqual(beforePreview);
  });

  it("retains authored query order across reorder, removal and reinsertion", () => {
    const adapter = new GraphPresentationAdapter(),
      view = new RetainedGraphPresentation();
    const a = node("a"),
      b = node("b");
    synchronize(view, adapter.update([a, b], []));
    const measured = view.statistics.measuredNodes;
    synchronize(view, adapter.update([b, a], []));
    expect(view.query(box(0))).toEqual([b, a]);
    expect(view.statistics.measuredNodes).toBe(measured);
    synchronize(view, adapter.update([b], []));
    synchronize(view, adapter.update([a, b], []));
    expect(view.query(box(0))).toEqual([a, b]);
    expect(view.all()).toEqual([a, b]);
    expect(view.all()).toBe(view.all());
  });

  it("recovers skipped deltas and replacement sources without retaining deleted identities", () => {
    const adapter = new GraphPresentationAdapter(),
      view = new RetainedGraphPresentation();
    synchronize(view, adapter.update([node("a"), node("b", 100)], []));
    adapter.update([node("a", 20)], []);
    const latest = adapter.update([node("a", 20)], [], 1);
    synchronize(view, latest);
    expect(view.size).toBe(1);
    expect(view.sourceNode("b")).toBeUndefined();
    expect(view.query(box(20)).map((value) => value.id)).toEqual(["a"]);
    synchronize(
      view,
      new GraphPresentationAdapter().update([node("replacement")], []),
    );
    expect(view.nodesById.has("a")).toBe(false);
    expect(view.all().map((value) => value.id)).toEqual(["replacement"]);
  });

  it("remeasures endpoint socket expansion and explicit local layout changes", () => {
    const adapter = new GraphPresentationAdapter(),
      view = new RetainedGraphPresentation();
    const nodes = [node("a"), node("b", 100)];
    const first = adapter.update(nodes, []);
    synchronize(view, first);
    const connected = adapter.update(nodes, [
      {
        from: { nodeId: "a", socketId: "out" },
        to: { nodeId: "b", socketId: "in" },
      },
    ]);
    view.synchronize(
      connected,
      overlays(),
      () => 2,
      () => 10,
      new Set(),
    );
    expect(view.query(box(0, 9))).toEqual([nodes[0]]);
    const before = view.statistics;
    view.synchronize(
      connected,
      overlays(),
      () => 2,
      () => 20,
      new Set(["a"]),
    );
    expect(view.query(box(0, 19))).toEqual([nodes[0]]);
    expect(view.statistics.measuredNodes - before.measuredNodes).toBe(1);
  });

  it("keeps preview precedence across source acknowledgement and automatic sizing", () => {
    const adapter = new GraphPresentationAdapter(),
      view = new RetainedGraphPresentation();
    const changes = overlays();
    changes.optimisticPositions.a = { x: 10, y: 0 };
    changes.dragPositions.a = { x: 20, y: 0 };
    changes.optimisticSizes.a = null;
    changes.optimisticLabels.a = "Renamed";
    const first = adapter.update(
      [{ ...node("a"), size: { width: 4, height: 4 } }],
      [],
    );
    synchronize(view, first, changes);
    expect(view.nodesById.get("a")).toMatchObject({
      position: { x: 20, y: 0 },
      size: undefined,
      label: "Renamed",
    });
    const acknowledged = adapter.update(
      [{ ...node("a", 10), label: "Renamed" }],
      [],
    );
    synchronize(view, acknowledged, changes);
    expect(view.nodesById.get("a")?.position.x).toBe(20);
    synchronize(view, acknowledged);
    expect(view.nodesById.get("a")).toBe(acknowledged.nodes[0]);
  });
});
