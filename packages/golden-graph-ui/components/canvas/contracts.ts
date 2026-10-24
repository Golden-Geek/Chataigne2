import type { Snippet } from "svelte";
import type {
  GraphCamera,
  GraphConnectionRequest,
  GraphNode,
  GraphNodeCreationRequest,
  GraphNodeMove,
  GraphNodePosition,
  GraphPresentationDocument,
  GraphNodeResize,
  GraphNodeSize,
  GraphSocket,
  GraphSocketRef,
  GraphViewportInset,
  GraphWorldBounds,
  GraphWorldContentContext,
} from "../../types";

export interface PanGesture {
  pointerId: number;
  startX: number;
  startY: number;
  cameraX: number;
  cameraY: number;
  moved: boolean;
  clearSelectionOnClick: boolean;
}

export interface NodeDragGesture {
  pointerId: number;
  startX: number;
  startY: number;
  nodePositions: Record<string, GraphNodePosition>;
}

export interface NodeResizeGesture {
  pointerId: number;
  nodeId: string;
  startX: number;
  startY: number;
  startSize: GraphNodeSize;
}

export interface SelectionGesture {
  pointerId: number;
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  initialSelectedNodeIds: string[];
}

export interface ConnectionDraft {
  pointerId: number;
  from: GraphSocketRef;
  endX: number;
  endY: number;
  snapNodeId?: string;
  snapSocketId?: string;
}

export const DEFAULT_MIN_ZOOM = 0.2;

export const DEFAULT_MAX_ZOOM = 2.5;

export const CHECKER_CELL_REM = 2;

export const DEFAULT_NODE_WIDTH_REM = 13;

export const MIN_NODE_WIDTH_REM = 8;

export const NODE_BORDER_REM = 0.08;

export const NODE_HEADER_REM = 1.8;

export const SOCKET_ROW_REM = 1.35;

export const HEADER_SOCKET_INSET_REM = 0.47;

export const FRAME_PADDING_REM = 3;

export const CAMERA_ANIMATION_MS = 240;

export const SCROLL_TARGET_ACTIVATION_MS = 300;

export const TITLE_DOUBLE_CLICK_MS = 300;

export const TITLE_DOUBLE_CLICK_DISTANCE_PX = 6;

export const finiteNumber = (
  value: number | undefined,
  fallback: number,
): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;

export const clamp = (
  value: number,
  minimum: number,
  maximum: number,
): number => Math.min(maximum, Math.max(minimum, value));

export interface GraphCanvasProps {
  graphDocument: GraphPresentationDocument;
  selectedNodeIds?: string[];
  selectedEdgeIds?: string[];
  onGraphSelectionChange?: (nodeIds: string[], edgeIds: string[]) => void;
  onNodeMove?: (
    nodeId: string,
    position: GraphNodePosition,
  ) => void | Promise<void>;
  onNodesMove?: (moves: GraphNodeMove[]) => void | Promise<void>;
  onNodeResize?: (resize: GraphNodeResize) => void | Promise<void>;
  onNodeRename?: (nodeId: string, label: string) => void | Promise<void>;
  onNodeCollapsedChange?: (
    nodeId: string,
    collapsed: boolean,
  ) => void | Promise<void>;
  onNodeEnabledChange?: (
    nodeId: string,
    enabled: boolean,
  ) => void | Promise<void>;
  onConnect?: (connection: GraphConnectionRequest) => void;
  canConnect?: (connection: GraphConnectionRequest) => boolean;
  nodeContent?: Snippet<[GraphNode]>;
  nodeHeaderContent?: Snippet<[GraphNode]>;
  inputSocketContent?: Snippet<[GraphNode, GraphSocket]>;
  outputSocketContent?: Snippet<[GraphNode, GraphSocket]>;
  worldContent?: Snippet<[GraphWorldContentContext]>;
  worldBounds?: GraphWorldBounds;
  toolbarEnd?: Snippet;
  onNodeContextMenu?: (event: MouseEvent, nodeId: string) => void;
  onBackgroundContextMenu?: (
    event: MouseEvent,
    position: GraphNodePosition,
  ) => void;
  onBackgroundDoubleClick?: (
    event: MouseEvent,
    position: GraphNodePosition,
  ) => void;
  onCreateRequest?: (request: GraphNodeCreationRequest) => void;
  routeEdgesAroundNodes?: boolean;
  socketLabels?: "always" | "hover" | "never";
  initialCamera?: GraphCamera;
  onCameraChange?: (camera: GraphCamera) => void;
  onVisibleNodeIdsChange?: (nodeIds: string[]) => void;
  viewportInset?: GraphViewportInset;
  minZoom?: number;
  maxZoom?: number;
  autoHomeOnMount?: boolean;
  emptyLabel?: string;
}

export type CanvasProps = GraphCanvasProps &
  Required<
    Pick<
      GraphCanvasProps,
      | "selectedNodeIds"
      | "selectedEdgeIds"
      | "routeEdgesAroundNodes"
      | "socketLabels"
      | "viewportInset"
      | "minZoom"
      | "maxZoom"
      | "autoHomeOnMount"
      | "emptyLabel"
    >
  >;
