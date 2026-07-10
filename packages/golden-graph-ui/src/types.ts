type Brand<Value, Name extends string> = Value & { readonly __brand: Name };

export type GraphId = Brand<string, "GraphId">;
export type GraphNodeId = Brand<string, "GraphNodeId">;
export type GraphPortId = Brand<string, "GraphPortId">;
export type GraphEdgeId = Brand<string, "GraphEdgeId">;

export interface Point {
  x: number;
  y: number;
}

export interface Rect extends Point {
  width: number;
  height: number;
}

export interface GraphNodeRecord<NodeData> {
  id: GraphNodeId;
  data: NodeData;
}

export interface PortRef {
  node: GraphNodeId;
  port: GraphPortId;
}

export interface GraphEdgeRecord<EdgeData> {
  id: GraphEdgeId;
  from: PortRef;
  to: PortRef;
  data: EdgeData;
}

export interface GraphCommentRecord {
  id: string;
  text: string;
  geometry: Rect;
}

export interface GraphGroupRecord {
  id: string;
  label: string;
  nodes: readonly GraphNodeId[];
}

export interface GraphSnapshot<NodeData, EdgeData> {
  graphId: GraphId;
  revision: number;
  nodes: readonly GraphNodeRecord<NodeData>[];
  edges: readonly GraphEdgeRecord<EdgeData>[];
  geometry: Readonly<Record<string, Rect>>;
  comments?: readonly GraphCommentRecord[];
  groups?: readonly GraphGroupRecord[];
}

export type GraphDeltaChange<NodeData, EdgeData> =
  | { kind: "node-inserted"; node: GraphNodeRecord<NodeData>; geometry?: Rect }
  | { kind: "node-removed"; nodeId: GraphNodeId }
  | { kind: "node-replaced"; node: GraphNodeRecord<NodeData> }
  | { kind: "edge-inserted"; edge: GraphEdgeRecord<EdgeData> }
  | { kind: "edge-removed"; edgeId: GraphEdgeId }
  | { kind: "node-geometry"; nodeId: GraphNodeId; geometry: Rect }
  | { kind: "comment-upserted"; comment: GraphCommentRecord }
  | { kind: "comment-removed"; commentId: string }
  | { kind: "group-upserted"; group: GraphGroupRecord }
  | { kind: "group-removed"; groupId: string };

export interface GraphDelta<NodeData, EdgeData> {
  before: number;
  after: number;
  changes: readonly GraphDeltaChange<NodeData, EdgeData>[];
}

export interface GraphPortView<PortData> {
  id: GraphPortId;
  direction: "input" | "output";
  data: PortData;
}

export interface GraphUiDomain<NodeData, PortData> {
  nodeLabel(node: NodeData): string;
  nodeKind(node: NodeData): string;
  nodePorts(node: NodeData): readonly GraphPortView<PortData>[];
}
