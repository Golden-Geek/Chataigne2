import { describe, expect, it } from "vitest";
import { chooseGraphEndpoint, graphKeyboardMoves } from "../keyboard";
import type { GraphNode } from "../types";

const node = (id: string, x = 0, y = 0): GraphNode => ({
  id,
  label: id,
  inputs: [],
  outputs: [],
  position: { x, y },
});
describe("shared graph keyboard intents", () => {
  it("forms the same directed request for either endpoint order", () => {
    const from = { nodeId: "a", socketId: "out", direction: "output" as const };
    const to = { nodeId: "b", socketId: "in", direction: "input" as const };
    const expected = {
      endpoint: null,
      connection: {
        from: { nodeId: "a", socketId: "out" },
        to: { nodeId: "b", socketId: "in" },
      },
    };
    expect(chooseGraphEndpoint(from, to)).toEqual(expected);
    expect(chooseGraphEndpoint(to, from)).toEqual(expected);
    expect(chooseGraphEndpoint(null, to)).toEqual({
      endpoint: to,
      connection: null,
    });
  });
  it("toggles the same endpoint and replaces another endpoint on the same side", () => {
    const first = { nodeId: "a", socketId: "x", direction: "output" as const };
    const next = { ...first, socketId: "y" };
    expect(chooseGraphEndpoint(first, first)).toEqual({
      endpoint: null,
      connection: null,
    });
    expect(chooseGraphEndpoint(first, next)).toEqual({
      endpoint: next,
      connection: null,
    });
  });
  it("moves all selected nodes in world rem units, deduplicates IDs and ignores missing nodes", () => {
    const lookup = new Map([
      ["a", node("a", -2, 3)],
      ["b", node("b", 10, 4)],
    ]);
    expect(
      graphKeyboardMoves(
        ["a", "b", "a", "missing"],
        (id) => lookup.get(id),
        "ArrowRight",
      ),
    ).toEqual([
      { nodeId: "a", position: { x: -1.75, y: 3 } },
      { nodeId: "b", position: { x: 10.25, y: 4 } },
    ]);
    expect(
      graphKeyboardMoves(["a"], (id) => lookup.get(id), "ArrowUp", true),
    ).toEqual([{ nodeId: "a", position: { x: -2, y: 1 } }]);
    expect(graphKeyboardMoves(["a"], (id) => lookup.get(id), "Enter")).toEqual(
      [],
    );
    expect(
      graphKeyboardMoves(["a"], () => node("a", NaN), "ArrowDown"),
    ).toEqual([]);
  });
  it("reads only selected nodes in a 100k graph", () => {
    const graph = new Map(
      Array.from({ length: 100000 }, (_, id) => [String(id), node(String(id))]),
    );
    let reads = 0;
    const moves = graphKeyboardMoves(
      ["7", "1234", "99999"],
      (id) => {
        reads++;
        return graph.get(id);
      },
      "ArrowLeft",
    );
    expect(reads).toBe(3);
    expect(moves).toHaveLength(3);
  });
});
