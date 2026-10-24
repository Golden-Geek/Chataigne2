import { sameNodeGeometry } from "./presentation-document";
import {
  projectEffectiveNode,
  type GraphNodePresentationChanges,
} from "./presentation-projection";
import { SpatialIndex, type SpatialBounds } from "./spatial-index";
import type { GraphNode, GraphPresentationDocument } from "./types";

export interface GraphPresentationWork {
  sourceNodes: number;
  projectedNodes: number;
  measuredNodes: number;
  spatialUpdates: number;
  materializedNodes: number;
  resets: number;
}

export interface GraphGeometryDelta {
  reset: boolean;
  updated: { id: string; bounds: SpatialBounds }[];
  removed: string[];
}

const emptyWork = (): GraphPresentationWork => ({
  sourceNodes: 0,
  projectedNodes: 0,
  measuredNodes: 0,
  spatialUpdates: 0,
  materializedNodes: 0,
  resets: 0,
});

/** Retains effective identities and spatial membership across source and sparse preview edits. */
export class RetainedGraphPresentation {
  #source: object | null = null;
  #sequence = -1;
  #geometryRevision = 0;
  readonly #sourceNodes = new Map<string, GraphNode>();
  readonly #nodes = new Map<string, GraphNode>();
  #spatial = new SpatialIndex<GraphNode>();
  #order = new Map<string, number>();
  #overlayIds = new Set<string>();
  #all: GraphNode[] | null = null;
  readonly #work = emptyWork();

  get size(): number {
    return this.#nodes.size;
  }
  get nodesById(): ReadonlyMap<string, GraphNode> {
    return this.#nodes;
  }
  get geometryRevision(): number {
    return this.#geometryRevision;
  }
  get statistics(): Readonly<GraphPresentationWork> {
    return { ...this.#work };
  }
  sourceNode(id: string): GraphNode | undefined {
    return this.#sourceNodes.get(id);
  }

  synchronize(
    document: GraphPresentationDocument,
    changes: Readonly<GraphNodePresentationChanges>,
    width: (node: GraphNode) => number,
    height: (node: GraphNode) => number,
    layoutDirtyIds: ReadonlySet<string>,
  ): GraphGeometryDelta {
    const replaced =
      this.#source !== document.source ||
      document.revision.sequence < this.#sequence;
    const changed = replaced || document.revision.sequence !== this.#sequence;
    const recover =
      changed && (replaced || document.changes.fromSequence !== this.#sequence);
    if (replaced) {
      this.#sourceNodes.clear();
      this.#nodes.clear();
      this.#spatial = new SpatialIndex();
      this.#overlayIds.clear();
      this.#all = null;
      this.#work.resets += 1;
    }
    const affected = new Set<string>(layoutDirtyIds);
    const delta: GraphGeometryDelta = {
      reset: replaced,
      updated: [],
      removed: [],
    };
    let geometryChanged = replaced;
    if (changed) {
      const updates = recover ? document.nodes : document.changes.updatedNodes;
      const retainedIds = recover
        ? new Set(document.nodes.map((node) => node.id))
        : null;
      const removed = recover
        ? [...this.#sourceNodes.keys()].filter((id) => !retainedIds!.has(id))
        : document.changes.removedNodeIds;
      for (const id of removed) {
        delta.removed.push(id);
        this.#sourceNodes.delete(id);
        this.#nodes.delete(id);
        this.#spatial.remove(id);
        this.#all = null;
        geometryChanged = true;
      }
      for (const node of updates) {
        this.#sourceNodes.set(node.id, node);
        affected.add(node.id);
        this.#work.sourceNodes += 1;
      }
      for (const id of document.changes.geometryNodeIds) affected.add(id);
      const order = recover
        ? document.nodes.map((node) => node.id)
        : document.changes.nodeOrder;
      if (order) {
        this.#order = new Map(order.map((id, index) => [id, index]));
        this.#all = null;
      }
      this.#source = document.source;
      this.#sequence = document.revision.sequence;
    }
    const overlayIds = new Set(
      Object.values(changes).flatMap((values) => Object.keys(values)),
    );
    for (const id of this.#overlayIds) affected.add(id);
    for (const id of overlayIds) affected.add(id);
    this.#overlayIds = overlayIds;
    const forced = new Set(layoutDirtyIds);
    if (recover) for (const node of document.nodes) forced.add(node.id);
    if (changed)
      for (const id of document.changes.geometryNodeIds) forced.add(id);
    for (const id of affected) {
      const source = this.#sourceNodes.get(id);
      if (!source) continue;
      const previous = this.#nodes.get(id);
      const next = projectEffectiveNode(source, changes);
      this.#work.projectedNodes += 1;
      if (!previous || forced.has(id) || !sameNodeGeometry(previous, next)) {
        const bounds = {
          left: next.position.x,
          top: next.position.y,
          right: next.position.x + width(next),
          bottom: next.position.y + height(next),
        };
        this.#work.measuredNodes += 1;
        delta.updated.push({ id, bounds });
        this.#work.spatialUpdates += 1;
        if (previous) this.#spatial.update(id, bounds, next);
        else this.#spatial.insert(id, bounds, next);
        geometryChanged = true;
      } else if (previous !== next) {
        this.#spatial.setValue(id, next);
      }
      if (previous !== next) {
        this.#nodes.set(id, next);
        this.#all = null;
      }
    }
    if (geometryChanged) this.#geometryRevision += 1;
    return delta;
  }

  query(bounds: SpatialBounds): GraphNode[] {
    return this.#spatial
      .query(bounds)
      .sort(
        (left, right) => this.#order.get(left.id)! - this.#order.get(right.id)!,
      );
  }

  /** Explicit whole-document operations such as Home may materialize authored order. */
  all(): GraphNode[] {
    if (!this.#all) {
      this.#all = [...this.#order.keys()].map((id) => this.#nodes.get(id)!);
      this.#work.materializedNodes += this.#all.length;
    }
    return this.#all;
  }
}
