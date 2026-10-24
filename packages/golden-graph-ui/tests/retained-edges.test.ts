import { describe, expect, it } from "vitest";
import { GraphPresentationAdapter } from "../presentation-document";
import { RetainedGraphPresentation } from "../retained-presentation";
import { RetainedEdgePresentation } from "../retained-edges";
import type { GraphNodePresentationChanges } from "../presentation-projection";
import type { GraphEdge, GraphNode } from "../types";

const node = (id: string, x: number, y = 0): GraphNode => ({
  id,
  label: id,
  position: { x, y },
  inputs: [],
  outputs: [],
});
const edge = (from: string, to: string, id = `${from}-${to}`): GraphEdge => ({
  id,
  from: { nodeId: from, socketId: "out" },
  to: { nodeId: to, socketId: "in" },
});
const emptyChanges = (): GraphNodePresentationChanges => ({
  dragPositions: {},
  optimisticPositions: {},
  resizeSizes: {},
  optimisticSizes: {},
  optimisticLabels: {},
  optimisticCollapsed: {},
  optimisticEnabled: {},
});
const viewBounds = { left: -10, top: -30, right: 300, bottom: 30 };
const fixture = () => {
  let adapter = new GraphPresentationAdapter();
  const projection = new RetainedGraphPresentation(),
    routes = new RetainedEdgePresentation();
  const update = (
    nodes: GraphNode[],
    edges: GraphEdge[],
    changes = emptyChanges(),
  ) => {
    const document = adapter.update(nodes, edges);
    const delta = projection.synchronize(
      document,
      changes,
      () => 2,
      () => 2,
      new Set(),
    );
    routes.synchronize(document, delta, (reference, direction) => {
      const node = projection.nodesById.get(reference.nodeId);
      return node
        ? {
            x: node.position.x + (direction === "output" ? 2 : 0),
            y: node.position.y + (reference.socketId === "second" ? 0.5 : 1),
          }
        : null;
    });
    return document;
  };
  const settle = (bounds = viewBounds) => {
    routes.visible(bounds, true, false);
    let frames = 0;
    while (routes.pending) {
      routes.processPending();
      if (++frames > 1024) throw new Error("Routing failed to settle.");
    }
    return routes.visible(bounds, true, false);
  };
  const replace = (nodes: GraphNode[], edges: GraphEdge[]) => {
    adapter = new GraphPresentationAdapter();
    return update(nodes, edges);
  };
  return { update, routes, projection, settle, replace };
};

describe("retained graph wire geometry", () => {
  it("retains authored order with shared endpoints and loops across edge reordering", () => {
    const { update, routes } = fixture();
    const nodes = [node("a", 0), node("b", 8)],
      edges = [edge("b", "a"), edge("a", "a"), edge("a", "b")];
    update(nodes, edges);
    expect(
      routes.visible(viewBounds, false, false).map((value) => value.edge.id),
    ).toEqual(edges.map((value) => value.id));
    update(nodes, [edges[2], edges[0], edges[1]]);
    expect(
      routes.visible(viewBounds, false, false).map((value) => value.edge.id),
    ).toEqual([edges[2].id, edges[0].id, edges[1].id]);
  });

  it("retires cached endpoint/source identities when ports or document producers change", () => {
    const { update, routes, settle, replace } = fixture();
    const nodes = [node("a", 0), node("b", 30)],
      connection = edge("a", "b");
    update(nodes, [connection]);
    settle();
    const first = routes.path(0, 1),
      before = routes.statistics;
    update(nodes, [
      { ...connection, to: { ...connection.to, socketId: "second" } },
    ]);
    settle();
    expect(routes.path(0, 1)).not.toBe(first);
    expect(
      routes.statistics.edgeGeometryUpdates - before.edgeGeometryUpdates,
    ).toBe(1);
    replace([node("a", 10), node("b", 40)], [connection]);
    settle();
    expect(routes.path(0, 1)).not.toBe(first);
    expect(routes.statistics.cachedRoutes).toBe(1);
  });
  it("keeps a crossing wire with both endpoints offscreen and culls distant paths", () => {
    const { update, routes } = fixture();
    update(
      [
        node("left", -40),
        node("right", 40),
        node("above-left", -40, 100),
        node("above-right", 40, 100),
      ],
      [edge("left", "right"), edge("above-left", "above-right")],
    );
    expect(
      routes
        .visible({ left: 0, top: 0, right: 10, bottom: 10 }, false, false)
        .map((value) => value.index),
    ).toEqual([0]);
    expect(routes.path(0, 1)).toContain("C");
  });

  it("invalidates changed obstacle search regions while preserving unrelated cached routes", () => {
    const { update, routes, settle } = fixture();
    const a = node("a", 0),
      b = node("b", 30),
      c = node("c", 200),
      d = node("d", 230),
      obstacle = node("obstacle", 15);
    const edges = [edge("a", "b"), edge("c", "d")];
    update([a, b, c, d, obstacle], edges);
    settle();
    const detour = routes.path(0, 1),
      unrelated = routes.path(1, 1),
      baseline = routes.statistics;
    update([a, b, c, d, node("obstacle", 15, 12)], edges);
    expect(
      routes.statistics.invalidatedRoutes - baseline.invalidatedRoutes,
    ).toBe(1);
    expect(routes.statistics.edgeGeometryUpdates).toBe(
      baseline.edgeGeometryUpdates,
    );
    expect(routes.path(1, 1)).toBe(unrelated);
    settle();
    expect(routes.path(0, 1)).not.toBe(detour);
    update([a, b, c, d], edges);
    settle();
    expect(routes.path(1, 1)).toBe(unrelated);
  });

  it.each([1000, 100000])(
    "updates a fixed endpoint delta locally among %i nodes and chain edges",
    (count) => {
      const { update, routes, projection, settle } = fixture();
      const nodes = Array.from({ length: count }, (_, index) =>
        node(String(index), index * 30),
      );
      const edges = nodes
        .slice(1)
        .map((value, index) => edge(String(index), value.id));
      update(nodes, edges);
      settle({ left: -5, top: -5, right: 10, bottom: 5 });
      const baseline = routes.statistics,
        projected = projection.statistics;
      const changes = emptyChanges();
      changes.dragPositions["0"] = { x: 1, y: 0 };
      update(nodes, edges, changes);
      expect(routes.statistics.obstacleUpdates - baseline.obstacleUpdates).toBe(
        1,
      );
      expect(
        routes.statistics.edgeGeometryUpdates - baseline.edgeGeometryUpdates,
      ).toBe(1);
      expect(
        routes.statistics.invalidatedRoutes - baseline.invalidatedRoutes,
      ).toBe(1);
      expect(
        projection.statistics.materializedNodes - projected.materializedNodes,
      ).toBe(0);
      settle({ left: -5, top: -5, right: 10, bottom: 5 });
      const settled = routes.statistics;
      update(nodes, edges);
      settle({ left: -5, top: -5, right: 10, bottom: 5 });
      expect(routes.statistics.obstacleUpdates - settled.obstacleUpdates).toBe(
        1,
      );
      expect(
        projection.statistics.materializedNodes - projected.materializedNodes,
      ).toBe(0);
    },
  );

  it("keeps unrelated cached paths during previews and ignores activity-only geometry invalidation", () => {
    const { update, routes, settle } = fixture();
    const nodes = [node("a", 0), node("b", 30), node("c", 200), node("d", 230)],
      edges = [edge("a", "b"), edge("c", "d")];
    update(nodes, edges);
    settle();
    const unchanged = routes.path(1, 1),
      before = routes.statistics;
    const changes = emptyChanges();
    changes.dragPositions.a = { x: 1, y: 0 };
    update(nodes, edges, changes);
    routes.visible(viewBounds, true, true);
    expect(routes.path(0, 1)).toContain("C");
    expect(routes.path(1, 1)).toBe(unchanged);
    expect(routes.pending).toBe(false);
    const preview = routes.statistics;
    update([{ ...nodes[0], active: true }, ...nodes.slice(1)], edges, changes);
    expect(routes.statistics).toEqual(preview);
    expect(before.cachedRoutes).toBe(2);
  });

  it("bounds per-frame work and cache admission without an eviction/requeue loop", () => {
    const { update, routes } = fixture();
    update(
      [node("a", 0), node("b", 30)],
      Array.from({ length: 1100 }, (_, index) => edge("a", "b", String(index))),
    );
    expect(routes.visible(viewBounds, true, false)).toHaveLength(1100);
    let frames = 0;
    while (routes.pending) {
      const before = routes.statistics;
      routes.processPending();
      const after = routes.statistics;
      expect(after.routeJobs - before.routeJobs).toBeLessThanOrEqual(4);
      expect(after.searchVisits - before.searchVisits).toBeLessThanOrEqual(
        4096,
      );
      expect(
        after.smoothingChecks - before.smoothingChecks,
      ).toBeLessThanOrEqual(2048);
      expect(after.cachedRoutes).toBeLessThanOrEqual(1024);
      if (++frames > 512) throw new Error("Unbounded route queue.");
    }
    expect(routes.statistics.cachedRoutes).toBe(1024);
    expect(routes.path(1099, 1)).toContain("C");
    routes.visible(viewBounds, true, false);
    expect(routes.pending).toBe(false);
    const settled = routes.statistics;
    routes.processPending();
    expect(routes.statistics).toEqual(settled);
  });

  it("retains exterior self-loops, changing endpoints and replacement/removal correctness", () => {
    const { update, routes, settle } = fixture();
    update([node("a", 0)], [edge("a", "a")]);
    settle();
    expect(routes.path(0, 1)).toContain("L");
    const before = routes.path(0, 1);
    update([node("a", 5)], [edge("a", "a")]);
    settle();
    expect(routes.path(0, 1)).not.toBe(before);
    update([], []);
    expect(routes.visible(viewBounds, true, false)).toEqual([]);
    expect(routes.statistics.cachedRoutes).toBe(0);
  });
});
