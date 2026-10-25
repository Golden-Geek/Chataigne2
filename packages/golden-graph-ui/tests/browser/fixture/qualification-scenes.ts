import type { GraphEdge, GraphNode } from "../../../types";

export function qualificationScene(count: number) {
  const node = (id: string, x: number, y: number): GraphNode => ({
    id,
    label: id,
    position: { x, y },
    size: { width: 13, height: 5 },
    inputs: [{ id: "in", label: "Input" }],
    outputs: [{ id: "out", label: "Output" }],
  });
  const nodes = Array.from({ length: count - 4 }, (_, index) =>
    node(`n${index}`, (index % 64) * 16, Math.floor(index / 64) * 6),
  );
  const edges: GraphEdge[] = nodes.slice(1).map((value, index) => ({
    id: `e${index}`,
    from: { nodeId: `n${index}`, socketId: "out" },
    to: { nodeId: value.id, socketId: "in" },
  }));
  nodes.push(
    node("cross-left", -24, -50),
    node("cross-right", 112, -50),
    node("loop", 20, -30),
    node("obstacle", 40, -40),
  );
  edges.push(
    {
      id: "crossing",
      from: { nodeId: "cross-left", socketId: "out" },
      to: { nodeId: "cross-right", socketId: "in" },
    },
    {
      id: "loop-edge",
      from: { nodeId: "loop", socketId: "out" },
      to: { nodeId: "loop", socketId: "in" },
    },
  );
  return { nodes, edges };
}
