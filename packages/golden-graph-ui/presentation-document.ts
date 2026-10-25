import type {
  GraphEdge,
  GraphNode,
  GraphPresentationDocument,
  GraphPresentationRevision,
} from "./types";

const TOPOLOGY = 1;
const GEOMETRY = 2;
const PAYLOAD = 4;
const ACTIVITY = 8;
const PREVIEW = 16;

// Exhaustive policies make new presentation fields require an invalidation decision.
const NODE_PLANES = {
  id: TOPOLOGY,
  label: GEOMETRY | PAYLOAD,
  subtitle: GEOMETRY | PAYLOAD,
  description: GEOMETRY | PAYLOAD,
  canRename: PAYLOAD,
  collapsed: GEOMETRY | PAYLOAD,
  enabled: PAYLOAD,
  canDisable: PAYLOAD,
  color: PAYLOAD,
  socketPlacement: GEOMETRY,
  position: GEOMETRY,
  size: GEOMETRY,
  automaticSize: GEOMETRY,
  resizable: PAYLOAD,
  inputs: TOPOLOGY | GEOMETRY | PAYLOAD,
  outputs: TOPOLOGY | GEOMETRY | PAYLOAD,
  headerInputs: TOPOLOGY | GEOMETRY | PAYLOAD,
  bypassConnections: PAYLOAD,
  active: ACTIVITY,
  invalid: PAYLOAD,
  warning: PAYLOAD,
} satisfies Record<keyof GraphNode, number>;

const EDGE_PLANES = {
  id: TOPOLOGY,
  from: TOPOLOGY,
  to: TOPOLOGY,
  color: PAYLOAD,
  targetColor: PAYLOAD,
  active: ACTIVITY,
  invalid: PAYLOAD,
} satisfies Record<keyof GraphEdge, number>;

const sameValue = (left: unknown, right: unknown): boolean => {
  if (Object.is(left, right)) return true;
  if (!left || !right || typeof left !== "object" || typeof right !== "object")
    return false;
  if (Array.isArray(left) !== Array.isArray(right)) return false;
  const a = left as Record<string, unknown>;
  const b = right as Record<string, unknown>;
  const keys = Object.keys(a);
  return (
    keys.length === Object.keys(b).length &&
    keys.every((key) => Object.hasOwn(b, key) && sameValue(a[key], b[key]))
  );
};

const difference = <T extends object>(
  previous: T,
  next: T,
  policy: Record<keyof T, number>,
): number => {
  if (previous === next) return 0;
  let mask = 0;
  for (const key of Object.keys(policy) as (keyof T)[]) {
    if (!sameValue(previous[key], next[key])) mask |= policy[key];
  }
  return mask;
};

export const sameNodeGeometry = (
  previous: GraphNode,
  next: GraphNode,
): boolean => {
  if (previous === next) return true;
  return (Object.keys(NODE_PLANES) as (keyof GraphNode)[]).every(
    (key) =>
      !(NODE_PLANES[key] & GEOMETRY) || sameValue(previous[key], next[key]),
  );
};

const initialRevision = (): GraphPresentationRevision => ({
  sequence: 0,
  topology: 0,
  geometry: 0,
  payload: 0,
  activity: 0,
  preview: 0,
});

/** Adapts immutable domain-neutral view arrays; owns no backend editing policy. */
export class GraphPresentationAdapter {
  readonly #source = {};
  #nodes: readonly GraphNode[] = [];
  #edges: readonly GraphEdge[] = [];
  #nodesById = new Map<string, GraphNode>();
  #preview: unknown;
  #document: GraphPresentationDocument = {
    source: this.#source,
    revision: initialRevision(),
    changes: {
      fromSequence: 0,
      updatedNodes: [],
      removedNodeIds: [],
      geometryNodeIds: [],
      nodeOrder: [],
    },
    nodes: [],
    edges: [],
  };

  update(
    nodes: readonly GraphNode[],
    edges: readonly GraphEdge[],
    preview?: unknown,
  ): GraphPresentationDocument {
    if (
      nodes === this.#nodes &&
      edges === this.#edges &&
      Object.is(preview, this.#preview)
    )
      return this.#document;
    let mask = Object.is(preview, this.#preview) ? 0 : PREVIEW;
    const updatedNodes: GraphNode[] = [];
    const removedNodeIds: string[] = [];
    const geometryNodeIds = new Set<string>();
    let nodeOrder: string[] | null = null;
    let nextNodesById = this.#nodesById;
    if (nodes !== this.#nodes) {
      nextNodesById = new Map();
      let orderChanged = nodes.length !== this.#nodes.length;
      for (let index = 0; index < nodes.length; index += 1) {
        const node = nodes[index];
        if (nextNodesById.has(node.id))
          throw new Error(`Duplicate graph presentation node '${node.id}'.`);
        nextNodesById.set(node.id, node);
        orderChanged ||= node.id !== this.#nodes[index]?.id;
        const previous = this.#nodesById.get(node.id);
        const changed = previous
          ? difference(previous, node, NODE_PLANES)
          : TOPOLOGY | GEOMETRY | PAYLOAD | ACTIVITY;
        mask |= changed;
        if (changed) updatedNodes.push(node);
        if (changed & GEOMETRY) geometryNodeIds.add(node.id);
      }
      for (const id of this.#nodesById.keys()) {
        if (!nextNodesById.has(id)) removedNodeIds.push(id);
      }
      if (orderChanged) {
        mask |= TOPOLOGY;
        nodeOrder = nodes.map((node) => node.id);
      }
      if (removedNodeIds.length) mask |= GEOMETRY;
    }
    let retainedEdges = this.#document.edges;
    if (edges !== this.#edges) {
      const edgeViews: GraphEdge[] = [];
      for (
        let index = 0;
        index < Math.max(edges.length, this.#edges.length);
        index += 1
      ) {
        const previous = this.#edges[index];
        const edge = edges[index];
        const changed =
          previous && edge
            ? difference(previous, edge, EDGE_PLANES)
            : TOPOLOGY | PAYLOAD | ACTIVITY;
        mask |= changed;
        if (edge)
          edgeViews.push(
            changed ? edge : (this.#document.edges[index] ?? edge),
          );
        if (changed & TOPOLOGY) {
          mask |= GEOMETRY;
          for (const value of [previous, edge]) {
            if (value) {
              geometryNodeIds.add(value.from.nodeId);
              geometryNodeIds.add(value.to.nodeId);
            }
          }
        }
      }
      retainedEdges = edgeViews;
    }
    this.#nodes = nodes;
    this.#edges = edges;
    this.#nodesById = nextNodesById;
    this.#preview = preview;
    if (!mask) return this.#document;
    const previous = this.#document.revision;
    this.#document = {
      source: this.#source,
      revision: {
        sequence: previous.sequence + 1,
        topology: previous.topology + Number(Boolean(mask & TOPOLOGY)),
        geometry: previous.geometry + Number(Boolean(mask & GEOMETRY)),
        payload: previous.payload + Number(Boolean(mask & PAYLOAD)),
        activity: previous.activity + Number(Boolean(mask & ACTIVITY)),
        preview: previous.preview + Number(Boolean(mask & PREVIEW)),
      },
      changes: {
        fromSequence: previous.sequence,
        updatedNodes,
        removedNodeIds,
        geometryNodeIds: [...geometryNodeIds],
        nodeOrder,
      },
      nodes,
      edges: retainedEdges,
    };
    return this.#document;
  }
}
