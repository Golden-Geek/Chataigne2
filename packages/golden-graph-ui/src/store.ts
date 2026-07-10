import type {
  GraphDelta,
  GraphCommentRecord,
  GraphEdgeId,
  GraphEdgeRecord,
  GraphNodeId,
  GraphNodeRecord,
  GraphSnapshot,
  GraphGroupRecord,
  Rect,
} from "./types";

export class GraphRevisionConflict extends Error {}

export class GraphStore<NodeData, EdgeData> {
  readonly nodes = new Map<GraphNodeId, GraphNodeRecord<NodeData>>();
  readonly edges = new Map<GraphEdgeId, GraphEdgeRecord<EdgeData>>();
  readonly geometry = new Map<GraphNodeId, Rect>();
  readonly comments = new Map<string, GraphCommentRecord>();
  readonly groups = new Map<string, GraphGroupRecord>();
  readonly selection = new Set<GraphNodeId>();

  graphRevision: number;
  topologyRevision = 0;
  geometryRevision = 0;
  presentationRevision = 0;
  selectionRevision = 0;
  viewportRevision = 0;

  constructor(snapshot: GraphSnapshot<NodeData, EdgeData>) {
    this.graphRevision = snapshot.revision;
    for (const node of snapshot.nodes) this.nodes.set(node.id, node);
    for (const edge of snapshot.edges) this.edges.set(edge.id, edge);
    for (const [nodeId, geometry] of Object.entries(snapshot.geometry)) {
      this.geometry.set(nodeId as GraphNodeId, geometry);
    }
    for (const comment of snapshot.comments ?? []) this.comments.set(comment.id, comment);
    for (const group of snapshot.groups ?? []) this.groups.set(group.id, group);
  }

  apply(delta: GraphDelta<NodeData, EdgeData>): void {
    if (delta.before !== this.graphRevision || delta.after <= delta.before) {
      throw new GraphRevisionConflict(
        `expected graph revision ${this.graphRevision}, received ${delta.before} -> ${delta.after}`,
      );
    }
    let topologyChanged = false;
    let geometryChanged = false;
    let presentationChanged = false;
    for (const change of delta.changes) {
      switch (change.kind) {
        case "node-inserted":
          this.nodes.set(change.node.id, change.node);
          if (change.geometry) this.geometry.set(change.node.id, change.geometry);
          topologyChanged = true;
          geometryChanged ||= change.geometry !== undefined;
          break;
        case "node-removed":
          this.nodes.delete(change.nodeId);
          this.geometry.delete(change.nodeId);
          this.selection.delete(change.nodeId);
          topologyChanged = true;
          geometryChanged = true;
          break;
        case "node-replaced":
          this.nodes.set(change.node.id, change.node);
          topologyChanged = true;
          break;
        case "edge-inserted":
          this.edges.set(change.edge.id, change.edge);
          topologyChanged = true;
          break;
        case "edge-removed":
          this.edges.delete(change.edgeId);
          topologyChanged = true;
          break;
        case "node-geometry":
          if (!this.nodes.has(change.nodeId)) {
            throw new Error(`geometry references missing node ${change.nodeId}`);
          }
          this.geometry.set(change.nodeId, change.geometry);
          geometryChanged = true;
          break;
        case "comment-upserted":
          this.comments.set(change.comment.id, change.comment);
          presentationChanged = true;
          break;
        case "comment-removed":
          this.comments.delete(change.commentId);
          presentationChanged = true;
          break;
        case "group-upserted":
          this.groups.set(change.group.id, change.group);
          presentationChanged = true;
          break;
        case "group-removed":
          this.groups.delete(change.groupId);
          presentationChanged = true;
          break;
      }
    }
    this.graphRevision = delta.after;
    if (topologyChanged) this.topologyRevision += 1;
    if (geometryChanged) this.geometryRevision += 1;
    if (presentationChanged) this.presentationRevision += 1;
  }

  setSelection(nodes: Iterable<GraphNodeId>): void {
    const next = new Set(nodes);
    if (setsEqual(this.selection, next)) return;
    this.selection.clear();
    for (const node of next) this.selection.add(node);
    this.selectionRevision += 1;
  }

  markViewportChanged(): void {
    this.viewportRevision += 1;
  }
}

function setsEqual<T>(left: Set<T>, right: Set<T>): boolean {
  return left.size === right.size && [...left].every((value) => right.has(value));
}
