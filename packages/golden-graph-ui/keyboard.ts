import type {
  GraphConnectionRequest,
  GraphNode,
  GraphNodeMove,
  GraphSocketDirection,
  GraphSocketRef,
} from "./types";

export interface GraphKeyboardEndpoint extends GraphSocketRef {
  direction: GraphSocketDirection;
}

/** Endpoint choice is presentation state; the caller owns connection policy and mutation. */
export function chooseGraphEndpoint(
  pending: GraphKeyboardEndpoint | null,
  next: GraphKeyboardEndpoint,
): {
  endpoint: GraphKeyboardEndpoint | null;
  connection: GraphConnectionRequest | null;
} {
  if (!pending || pending.direction === next.direction)
    return {
      endpoint:
        pending?.nodeId === next.nodeId && pending.socketId === next.socketId
          ? null
          : next,
      connection: null,
    };
  const output = pending.direction === "output" ? pending : next;
  const input = pending.direction === "input" ? pending : next;
  return {
    endpoint: null,
    connection: {
      from: { nodeId: output.nodeId, socketId: output.socketId },
      to: { nodeId: input.nodeId, socketId: input.socketId },
    },
  };
}

/** Work is proportional to selected IDs, independently of total graph size and mounted rows. */
export function graphKeyboardMoves(
  ids: readonly string[],
  node: (id: string) => GraphNode | undefined,
  key: string,
  large = false,
): GraphNodeMove[] {
  const step = large ? 2 : 0.25;
  const delta =
    key === "ArrowLeft"
      ? [-step, 0]
      : key === "ArrowRight"
        ? [step, 0]
        : key === "ArrowUp"
          ? [0, -step]
          : key === "ArrowDown"
            ? [0, step]
            : null;
  if (!delta) return [];
  const moves: GraphNodeMove[] = [];
  for (const id of new Set(ids)) {
    const current = node(id);
    if (
      !current ||
      !Number.isFinite(current.position.x) ||
      !Number.isFinite(current.position.y)
    )
      continue;
    const position = {
      x: current.position.x + delta[0],
      y: current.position.y + delta[1],
    };
    if (Number.isFinite(position.x) && Number.isFinite(position.y))
      moves.push({ nodeId: id, position });
  }
  return moves;
}
