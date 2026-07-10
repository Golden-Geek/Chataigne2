import { SpatialIndex } from "./spatial-index";
import { GraphStore } from "./store";
import type {
  GraphDelta,
  GraphNodeId,
  GraphNodeRecord,
  GraphSnapshot,
  Rect,
} from "./types";

export interface VisibleGraphNode<NodeData> {
  node: GraphNodeRecord<NodeData>;
  geometry: Rect;
}

export class GraphEditorModel<NodeData, EdgeData> {
  readonly store: GraphStore<NodeData, EdgeData>;
  readonly spatialIndex: SpatialIndex;

  constructor(snapshot: GraphSnapshot<NodeData, EdgeData>, cellSize?: number) {
    this.store = new GraphStore(snapshot);
    this.spatialIndex = new SpatialIndex(cellSize);
    for (const [nodeId, geometry] of this.store.geometry) {
      this.spatialIndex.upsert(nodeId, geometry);
    }
  }

  apply(delta: GraphDelta<NodeData, EdgeData>): void {
    this.store.apply(delta);
    for (const change of delta.changes) {
      if (change.kind === "node-removed") this.spatialIndex.remove(change.nodeId);
      if (change.kind === "node-inserted" && change.geometry) {
        this.spatialIndex.upsert(change.node.id, change.geometry);
      }
      if (change.kind === "node-geometry") {
        this.spatialIndex.upsert(change.nodeId, change.geometry);
      }
    }
  }

  visibleNodes(viewport: Rect): VisibleGraphNode<NodeData>[] {
    return this.spatialIndex.query(viewport).flatMap((nodeId) => {
      const node = this.store.nodes.get(nodeId);
      const geometry = this.store.geometry.get(nodeId);
      return node && geometry ? [{ node, geometry }] : [];
    });
  }

  hitTest(x: number, y: number): readonly GraphNodeId[] {
    return this.spatialIndex.hitTest(x, y);
  }
}
