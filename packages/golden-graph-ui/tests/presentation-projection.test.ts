import { describe, expect, it } from "vitest";
import {
  projectEffectiveNode,
  viewportWorldBounds,
  type GraphNodePresentationChanges,
} from "../presentation-projection";
import type { GraphNode } from "../types";
import { GraphPresentationAdapter } from "../presentation-document";
import { RetainedGraphPresentation } from "../retained-presentation";

const node = (id: string, x: number): GraphNode => ({
  id,
  label: id,
  position: { x, y: 0 },
  inputs: [],
  outputs: [],
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

describe("generic graph presentation projection", () => {
  it("applies live interaction overrides ahead of optimistic values", () => {
    const nodes = [node("edited", 0), node("untouched", 100)];
    const changes = emptyChanges();
    changes.dragPositions.edited = { x: 3, y: 4 };
    changes.optimisticPositions.edited = { x: 8, y: 9 };
    changes.resizeSizes.edited = { width: 20, height: 10 };
    changes.optimisticSizes.edited = { width: 30, height: 15 };
    changes.optimisticLabels.edited = "Renamed";
    changes.optimisticCollapsed.edited = true;
    changes.optimisticEnabled.edited = false;

    const projected = projectEffectiveNode(nodes[0], changes);
    expect(projected).toMatchObject({
      position: { x: 3, y: 4 },
      size: { width: 20, height: 10 },
      label: "Renamed",
      collapsed: true,
      enabled: false,
    });
    expect(projectEffectiveNode(nodes[1], changes)).toBe(nodes[1]);
    expect(nodes[0].position).toEqual({ x: 0, y: 0 });
  });

  it("allows an optimistic automatic-size reset without mutating the source", () => {
    const source = { ...node("reset", 0), size: { width: 12, height: 8 } };
    const changes = emptyChanges();
    changes.optimisticSizes.reset = null;
    const projected = projectEffectiveNode(source, changes);
    expect(projected.size).toBeUndefined();
    expect(source.size).toEqual({ width: 12, height: 8 });
  });

  it("uses camera and viewport dimensions to query the spatial index", () => {
    const nodes = [node("left", -100), node("visible", 0), node("right", 100)];
    const index = new RetainedGraphPresentation();
    index.synchronize(
      new GraphPresentationAdapter().update(nodes, []),
      emptyChanges(),
      () => 4,
      () => 4,
      new Set(),
    );
    expect(index.size).toBe(3);
    const bounds = viewportWorldBounds(
      { x: 0, y: 0, zoom: 1 },
      160,
      160,
      16,
      0,
    );
    expect(bounds.left).toBeCloseTo(0);
    expect(bounds.top).toBeCloseTo(0);
    expect(bounds.right).toBe(10);
    expect(bounds.bottom).toBe(10);
    expect(index.query(bounds).map((entry) => entry.id)).toEqual(["visible"]);
    expect(
      viewportWorldBounds({ x: -32, y: 16, zoom: 2 }, 160, 160, 16, 8),
    ).toEqual({
      left: -7,
      top: -8.5,
      right: 14,
      bottom: 12.5,
    });
  });
});
