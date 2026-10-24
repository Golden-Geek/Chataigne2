import type { SpatialBounds } from "./spatial-index";
import type {
  GraphCamera,
  GraphNode,
  GraphNodePosition,
  GraphNodeSize,
} from "./types";

export interface GraphNodePresentationChanges {
  dragPositions: Record<string, GraphNodePosition>;
  optimisticPositions: Record<string, GraphNodePosition>;
  resizeSizes: Record<string, GraphNodeSize>;
  optimisticSizes: Record<string, GraphNodeSize | null>;
  optimisticLabels: Record<string, string>;
  optimisticCollapsed: Record<string, boolean>;
  optimisticEnabled: Record<string, boolean>;
}

export const projectEffectiveNode = (
  node: GraphNode,
  changes: Readonly<GraphNodePresentationChanges>,
): GraphNode => {
  const position =
    changes.dragPositions[node.id] ?? changes.optimisticPositions[node.id];
  const size =
    node.id in changes.resizeSizes
      ? changes.resizeSizes[node.id]
      : changes.optimisticSizes[node.id];
  const label = changes.optimisticLabels[node.id];
  const collapsed = changes.optimisticCollapsed[node.id];
  const enabled = changes.optimisticEnabled[node.id];
  return position !== undefined ||
    size !== undefined ||
    label !== undefined ||
    collapsed !== undefined ||
    enabled !== undefined
    ? {
        ...node,
        ...(position !== undefined ? { position } : {}),
        ...(size !== undefined ? { size: size ?? undefined } : {}),
        ...(label !== undefined ? { label } : {}),
        ...(collapsed !== undefined ? { collapsed } : {}),
        ...(enabled !== undefined ? { enabled } : {}),
      }
    : node;
};

export const viewportWorldBounds = (
  camera: GraphCamera,
  viewportWidth: number,
  viewportHeight: number,
  remPx: number,
  margin: number,
): SpatialBounds => ({
  left: -camera.x / camera.zoom / remPx - margin,
  top: -camera.y / camera.zoom / remPx - margin,
  right: (viewportWidth - camera.x) / camera.zoom / remPx + margin,
  bottom: (viewportHeight - camera.y) / camera.zoom / remPx + margin,
});
