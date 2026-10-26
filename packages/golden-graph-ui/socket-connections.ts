import type { GraphEdge, GraphSocketDirection } from "./types";

// Length-prefixing keeps arbitrary node/socket IDs distinct without separator collisions.
const key = (nodeId: string, socketId: string) =>
  `${nodeId.length}:${nodeId}${socketId}`;

/** Immutable endpoint occupancy; an input and output may legally share a socket ID. */
export class GraphSocketConnections {
  readonly #inputs = new Set<string>();
  readonly #outputs = new Set<string>();

  constructor(edges: readonly GraphEdge[]) {
    for (const edge of edges) {
      this.#outputs.add(key(edge.from.nodeId, edge.from.socketId));
      this.#inputs.add(key(edge.to.nodeId, edge.to.socketId));
    }
  }

  has(
    nodeId: string,
    socketId: string,
    direction: GraphSocketDirection,
  ): boolean {
    return (direction === "input" ? this.#inputs : this.#outputs).has(
      key(nodeId, socketId),
    );
  }
}
