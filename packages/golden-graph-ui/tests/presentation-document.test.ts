import { describe, expect, it } from "vitest";
import { GraphPresentationAdapter } from "../presentation-document";
import type { GraphEdge, GraphNode } from "../types";

const node = (id = "node"): GraphNode => ({
  id,
  label: id,
  position: { x: 0, y: 0 },
  inputs: [],
  outputs: [],
});
const edge = (from = "node", to = "other"): GraphEdge => ({
  id: "edge",
  from: { nodeId: from, socketId: "out" },
  to: { nodeId: to, socketId: "in" },
});

describe("presentation revision planes", () => {
  it("retains equivalent wire identities while publishing a changed peer", () => {
    const adapter = new GraphPresentationAdapter();
    const nodes = [node(), node("other")];
    const first = adapter.update(nodes, [edge(), { ...edge(), id: "peer" }]);
    const changed = adapter.update(nodes, [
      { ...edge() },
      { ...edge(), id: "peer", active: true },
    ]);
    expect(changed.edges[0]).toBe(first.edges[0]);
    expect(changed.edges[1]).not.toBe(first.edges[1]);
    expect(changed.revision.activity).toBe(first.revision.activity + 1);
    expect(changed.revision.geometry).toBe(first.revision.geometry);
  });
  it("retains a document across equivalent allocations", () => {
    const adapter = new GraphPresentationAdapter();
    const nodes = [node()];
    const first = adapter.update(nodes, []);
    expect(adapter.update(nodes, [])).toBe(first);
    expect(adapter.update([{ ...node(), position: { x: 0, y: 0 } }], [])).toBe(
      first,
    );
  });

  it("publishes activity and previews independently of topology and geometry", () => {
    const adapter = new GraphPresentationAdapter();
    const source = node();
    const first = adapter.update([source], [edge()]);
    const active = adapter.update(
      [{ ...source, active: true }],
      [{ ...edge(), active: true }],
    );
    expect(active.revision).toEqual({
      ...first.revision,
      sequence: first.revision.sequence + 1,
      activity: first.revision.activity + 1,
    });
    expect(active.changes.geometryNodeIds).toEqual([]);
    const preview = adapter.update(active.nodes, active.edges, {
      display: "latest",
    });
    expect(preview.revision).toEqual({
      ...active.revision,
      sequence: active.revision.sequence + 1,
      preview: active.revision.preview + 1,
    });
    expect(preview.changes.updatedNodes).toEqual([]);
  });

  it("separates payload from geometry while retaining geometry-affecting label changes", () => {
    const adapter = new GraphPresentationAdapter();
    const first = adapter.update([node()], []);
    const payload = adapter.update(
      [{ ...node(), color: "red", enabled: false, warning: "Warning" }],
      [],
    );
    expect(payload.revision).toEqual({
      ...first.revision,
      sequence: first.revision.sequence + 1,
      payload: first.revision.payload + 1,
    });
    const moved = adapter.update(
      [{ ...payload.nodes[0], position: { x: 10, y: 0 } }],
      [],
    );
    expect(moved.revision).toEqual({
      ...payload.revision,
      sequence: payload.revision.sequence + 1,
      geometry: payload.revision.geometry + 1,
    });
    expect(moved.changes.geometryNodeIds).toEqual(["node"]);
    const renamed = adapter.update(
      [{ ...moved.nodes[0], label: "A longer heading" }],
      [],
    );
    expect(renamed.revision.geometry).toBe(moved.revision.geometry + 1);
    expect(renamed.revision.payload).toBe(moved.revision.payload + 1);
    expect(renamed.revision.topology).toBe(moved.revision.topology);
  });

  it("publishes authored order, removals and connection-driven layout invalidation", () => {
    const adapter = new GraphPresentationAdapter();
    const a = node("a"),
      b = node("b");
    const first = adapter.update([a, b], []);
    const reordered = adapter.update([b, a], []);
    expect(reordered.changes.nodeOrder).toEqual(["b", "a"]);
    expect(reordered.changes.updatedNodes).toEqual([]);
    expect(reordered.revision.geometry).toBe(first.revision.geometry);
    const connected = adapter.update(reordered.nodes, [edge("a", "b")]);
    expect(new Set(connected.changes.geometryNodeIds)).toEqual(
      new Set(["a", "b"]),
    );
    const removed = adapter.update([a], []);
    expect(removed.changes.removedNodeIds).toEqual(["b"]);
    expect(removed.changes.fromSequence).toBe(connected.revision.sequence);
  });

  it("does not advance topology when only edge payload or activity changes", () => {
    const adapter = new GraphPresentationAdapter();
    const nodes = [node()];
    const first = adapter.update(nodes, [edge()]);
    const payload = adapter.update(nodes, [{ ...edge(), color: "blue" }]);
    expect(payload.revision.topology).toBe(first.revision.topology);
    expect(payload.revision.geometry).toBe(first.revision.geometry);
    expect(payload.revision.payload).toBe(first.revision.payload + 1);
  });

  it("rejects duplicate identities without poisoning the following update", () => {
    const adapter = new GraphPresentationAdapter();
    const nodes = [node()];
    const first = adapter.update(nodes, []);
    expect(() => adapter.update([node(), node()], [])).toThrow("Duplicate");
    expect(adapter.update(nodes, [])).toBe(first);
    expect(
      adapter.update([{ ...node(), active: true }], []).revision.sequence,
    ).toBe(first.revision.sequence + 1);
    expect(new GraphPresentationAdapter().update(nodes, []).source).not.toBe(
      first.source,
    );
  });
});
