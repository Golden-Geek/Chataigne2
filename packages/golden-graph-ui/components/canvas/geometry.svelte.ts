import {
  type ConnectionDraft,
  DEFAULT_NODE_WIDTH_REM,
  MIN_NODE_WIDTH_REM,
  NODE_BORDER_REM,
  NODE_HEADER_REM,
  SOCKET_ROW_REM,
  HEADER_SOCKET_INSET_REM,
  ROUTING_CLEARANCE_REM,
  clamp,
  type CanvasProps,
} from "./contracts";
import type {
  GraphConnectionRequest,
  GraphEdge,
  GraphNode,
  GraphNodeBypassConnection,
  GraphNodePosition,
  GraphNodeSize,
  GraphSocket,
  GraphSocketDirection,
  GraphSocketRef,
} from "../../types";
import { untrack } from "svelte";
import { RetainedGraphPresentation } from "../../retained-presentation";
import type { SpatialBounds } from "../../spatial-index";
import {
  indexGraphEdgesByNode,
  projectVisibleEdges,
  viewportWorldBounds,
} from "../../presentation-projection";
import {
  ROUTING_GRID_REM,
  routeEdgeAroundObstacles,
  routingObstacleBuckets,
  type RoutingObstacle,
} from "../../edge-routing";
import type { GraphNodePresentationChanges } from "../../presentation-projection";
import type { CanvasViewport } from "./viewport.svelte";

interface GeometryInteraction extends Readonly<GraphNodePresentationChanges> {
  readonly connectionDraft: ConnectionDraft | null;
}
export function createCanvasGeometry(
  props: CanvasProps,
  viewport: CanvasViewport,
  interaction: GeometryInteraction,
) {
  let nodes = $derived(props.graphDocument.nodes);

  let edges = $derived(props.graphDocument.edges);

  let expandedSockets = $state.raw<Record<string, boolean>>({});
  const layoutDirtyIds = new Set<string>();
  const projection = new RetainedGraphPresentation();
  const routedPathCache = new Map<string, string>();
  let cachedTopologySource: object | null = null;
  let cachedTopologyRevision = -1;
  let cachedTopology = {
    indexes: new Map<string, number[]>(),
    connectedSockets: new Set<string>(),
  };
  let topologyView = $derived.by(() => {
    const document = props.graphDocument;
    if (
      document.source !== cachedTopologySource ||
      document.revision.topology !== cachedTopologyRevision
    ) {
      cachedTopologySource = document.source;
      cachedTopologyRevision = document.revision.topology;
      cachedTopology = {
        indexes: indexGraphEdgesByNode(document.edges),
        connectedSockets: new Set(
          document.edges.flatMap((edge) => [
            `${edge.from.nodeId}:${edge.from.socketId}`,
            `${edge.to.nodeId}:${edge.to.socketId}`,
          ]),
        ),
      };
    }
    return cachedTopology;
  });
  let frame = $derived.by(() => {
    const document = props.graphDocument;
    const changes = {
      dragPositions: interaction.dragPositions,
      optimisticPositions: interaction.optimisticPositions,
      resizeSizes: interaction.resizeSizes,
      optimisticSizes: interaction.optimisticSizes,
      optimisticLabels: interaction.optimisticLabels,
      optimisticCollapsed: interaction.optimisticCollapsed,
      optimisticEnabled: interaction.optimisticEnabled,
    };
    expandedSockets;
    topologyView;
    return untrack(() => {
      projection.synchronize(
        document,
        changes,
        nodeWidth,
        nodeHeight,
        layoutDirtyIds,
      );
      layoutDirtyIds.clear();
      return { geometry: projection.geometryRevision };
    });
  });
  let geometryRevision = $derived(frame.geometry);
  let effectiveNodes = $derived.by(() => {
    frame;
    return projection.all();
  });
  const nodeById = (id: string): GraphNode | undefined => {
    frame;
    return projection.nodesById.get(id);
  };
  const sourceNode = (id: string): GraphNode | undefined => {
    frame;
    return projection.sourceNode(id);
  };
  const queryNodes = (bounds: SpatialBounds): GraphNode[] => {
    frame;
    return projection.query(bounds);
  };

  const nodeAutomaticSize = (node: GraphNode): GraphNodeSize | undefined => {
    const size = node.automaticSize;
    return size && size.width > 0 && size.height > 0 ? size : undefined;
  };

  const nodeWidth = (node: GraphNode): number =>
    Math.max(
      MIN_NODE_WIDTH_REM,
      node.size?.width ??
        nodeAutomaticSize(node)?.width ??
        DEFAULT_NODE_WIDTH_REM,
    );

  const headerSublineText = (node: GraphNode): string =>
    node.description?.trim() || node.subtitle?.trim() || "";

  const wrappedLineCount = (
    text: string,
    node: GraphNode,
    averageCharWidthRem: number,
  ): number => {
    const trimmed = text.trim();
    if (!trimmed) {
      return 0;
    }
    const socketSpace =
      (node.collapsed === true
        ? (allNodeSockets(node, "input").length > 0 ? 1.8 : 0) +
          (allNodeSockets(node, "output").length > 0 ? 1.8 : 0)
        : (node.socketPlacement === "header"
            ? node.inputs.length + node.outputs.length
            : 0) * 1.55) +
      ((node.headerInputs?.length ?? 0) > 0 && node.socketPlacement !== "header"
        ? 1.2
        : 0);
    const availableRem = Math.max(4.5, nodeWidth(node) - socketSpace - 0.9);
    const charsPerLine = Math.max(
      8,
      Math.floor(availableRem / averageCharWidthRem),
    );
    return Math.max(1, Math.ceil(trimmed.length / charsPerLine));
  };

  const nodeHeaderHeight = (node: GraphNode): number => {
    const titleLines = wrappedLineCount(node.label, node, 0.43);
    const sublineLines = wrappedLineCount(headerSublineText(node), node, 0.34);
    return Math.max(
      NODE_HEADER_REM,
      0.34 + Math.max(1, titleLines) * 0.82 + sublineLines * 0.66 + 0.32,
    );
  };

  const socketStart = (node: GraphNode): number =>
    nodeHeaderHeight(node) + 0.25 + NODE_BORDER_REM;

  const rootNodeSockets = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): GraphSocket[] =>
    direction === "input"
      ? [...node.inputs, ...(node.headerInputs ?? [])]
      : node.outputs;

  const flattenSockets = (sockets: readonly GraphSocket[]): GraphSocket[] =>
    sockets.flatMap((socket) => [
      socket,
      ...flattenSockets(socket.children ?? []),
    ]);

  const allNodeSockets = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): GraphSocket[] => flattenSockets(rootNodeSockets(node, direction));

  const socketRefKey = (nodeId: string, socketId: string): string =>
    `${nodeId}:${socketId}`;

  const socketExpansionKey = (
    node: GraphNode,
    socket: GraphSocket,
    direction: GraphSocketDirection,
  ): string => `${node.id}:${direction}:${socket.id}`;

  const hasSocketConnection = (node: GraphNode, socketId: string): boolean =>
    connectedSockets.has(socketRefKey(node.id, socketId));

  const socketHasConnectedChild = (
    node: GraphNode,
    socket: GraphSocket,
  ): boolean =>
    (socket.children ?? []).some((child) =>
      hasSocketConnection(node, child.id),
    );

  const socketExpanded = (
    node: GraphNode,
    socket: GraphSocket,
    direction: GraphSocketDirection,
  ): boolean =>
    (socket.children?.length ?? 0) > 0 &&
    (socketHasConnectedChild(node, socket) ||
      expandedSockets[socketExpansionKey(node, socket, direction)] === true);

  const displaySockets = (
    node: GraphNode,
    direction: GraphSocketDirection,
    sockets: readonly GraphSocket[],
  ): GraphSocket[] =>
    sockets.flatMap((socket) =>
      socketExpanded(node, socket, direction)
        ? [socket, ...(socket.children ?? [])]
        : [socket],
    );

  const displayNodeSockets = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): GraphSocket[] =>
    direction === "input"
      ? [
          ...displaySockets(node, "input", node.inputs),
          ...(node.headerInputs ?? []),
        ]
      : displaySockets(node, "output", node.outputs);

  const connectionAllowed = (connection: GraphConnectionRequest): boolean =>
    props.canConnect?.(connection) ?? true;

  const socketDisabled = (
    node: GraphNode,
    socket: GraphSocket,
    direction: GraphSocketDirection,
  ): boolean => {
    if (socket.compatible === false) return true;
    if (direction === "output") return false;
    if (socket.parentId && hasSocketConnection(node, socket.parentId))
      return true;
    if (!socket.parentId && socketHasConnectedChild(node, socket)) return true;
    if (interaction.connectionDraft) {
      return !connectionAllowed({
        from: interaction.connectionDraft.from,
        to: { nodeId: node.id, socketId: socket.id },
      });
    }
    return false;
  };

  const inputSocketContentVisible = (
    node: GraphNode,
    socket: GraphSocket,
  ): boolean =>
    Boolean(props.inputSocketContent) &&
    Boolean(socket.defaultParamId) &&
    !socket.parentId &&
    !hasSocketConnection(node, socket.id) &&
    !socketHasConnectedChild(node, socket);

  const socketDefaultSlotWidth = (socket: GraphSocket): number => {
    if (!socket.defaultParamId) return 0;
    switch ((socket.valueType ?? "").trim()) {
      case "vec3":
      case "color":
        return 8.2;
      case "vec2":
        return 6.6;
      case "bool":
        return 1.4;
      case "int":
      case "float":
      case "duration":
        return 8.8;
      default:
        return 5.4;
    }
  };

  const socketDefaultColumnWidth = (sockets: readonly GraphSocket[]): number =>
    sockets.reduce(
      (width, socket) => Math.max(width, socketDefaultSlotWidth(socket)),
      0,
    );

  const toggleSocketExpanded = (
    event: MouseEvent,
    node: GraphNode,
    socket: GraphSocket,
    direction: GraphSocketDirection,
  ): void => {
    event.preventDefault();
    event.stopPropagation();
    const key = socketExpansionKey(node, socket, direction);
    layoutDirtyIds.add(node.id);
    expandedSockets = {
      ...expandedSockets,
      [key]: !socketExpanded(node, socket, direction),
    };
  };

  const firstCollapsedSocket = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): GraphSocket | null => {
    const sockets = rootNodeSockets(node, direction);
    return (
      sockets.find((socket) => socket.compatible !== false) ??
      sockets[0] ??
      null
    );
  };

  const collapsedSocketLabel = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): string => {
    const sockets = rootNodeSockets(node, direction);
    if (sockets.length === 1) {
      return sockets[0].label;
    }
    return direction === "input" ? "In" : "Out";
  };

  const collapsedSocketTitle = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): string => {
    const sockets = rootNodeSockets(node, direction);
    if (sockets.length === 1) {
      return sockets[0].valueType ?? sockets[0].label;
    }
    return `${sockets.length} ${direction === "input" ? "inputs" : "outputs"}`;
  };

  const collapsedSocketConnected = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): boolean =>
    allNodeSockets(node, direction).some((socket) =>
      connectedSockets.has(`${node.id}:${socket.id}`),
    );

  const collapsedSocketDraftTarget = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): boolean =>
    direction === "input" &&
    interaction.connectionDraft?.snapNodeId === node.id &&
    allNodeSockets(node, direction).some(
      (socket) => interaction.connectionDraft?.snapSocketId === socket.id,
    );

  const minimumNodeHeight = (node: GraphNode): number =>
    node.collapsed === true
      ? nodeHeaderHeight(node) + NODE_BORDER_REM * 2
      : node.socketPlacement === "header"
        ? nodeHeaderHeight(node) + NODE_BORDER_REM * 2 + 1.2
        : Math.max(
            nodeHeaderHeight(node) + NODE_BORDER_REM * 2 + 1.2,
            nodeHeaderHeight(node) +
              NODE_BORDER_REM * 2 +
              0.8 +
              Math.max(
                displayNodeSockets(node, "input").filter(
                  (socket) => !node.headerInputs?.includes(socket),
                ).length,
                displayNodeSockets(node, "output").length,
                1,
              ) *
                SOCKET_ROW_REM,
          );

  const nodeHeight = (node: GraphNode): number =>
    node.collapsed === true
      ? minimumNodeHeight(node)
      : Math.max(
          minimumNodeHeight(node),
          node.size?.height ??
            nodeAutomaticSize(node)?.height ??
            minimumNodeHeight(node),
        );

  let routingGeometryKey = $derived.by(() =>
    !props.routeEdgesAroundNodes || edges.length === 0
      ? `${viewport.remPx}:inactive`
      : `${viewport.remPx}:${geometryRevision}`,
  );

  let appliedRoutingKey: string | null = null;

  let visibleNodes = $derived(
    queryNodes(
      viewportWorldBounds(
        viewport.camera,
        viewport.viewportWidth,
        viewport.viewportHeight,
        viewport.remPx,
        8,
      ),
    ),
  );

  let viewportVisibleNodes = $derived(
    queryNodes(
      viewportWorldBounds(
        viewport.camera,
        viewport.viewportWidth,
        viewport.viewportHeight,
        viewport.remPx,
        0,
      ),
    ),
  );

  let publishedVisibleNodeKey = "";

  $effect(() => {
    const nodeIds = viewportVisibleNodes.map((node) => node.id).sort();
    const key = nodeIds.join("\u0000");
    if (key === publishedVisibleNodeKey) return;
    publishedVisibleNodeKey = key;
    props.onVisibleNodeIdsChange?.(nodeIds);
  });

  let visibleNodeIds = $derived(new Set(visibleNodes.map((node) => node.id)));

  let visibleEdges = $derived(
    projectVisibleEdges(edges, topologyView.indexes, visibleNodeIds),
  );

  let connectedSockets = $derived(topologyView.connectedSockets);
  let edgeCount = $derived(edges.length);
  let routingPreview = $derived(
    Object.keys(interaction.dragPositions).length > 0 ||
      Object.keys(interaction.resizeSizes).length > 0,
  );

  let routingObstacles = $derived.by((): RoutingObstacle[] => {
    geometryRevision;
    if (!props.routeEdgesAroundNodes || edgeCount === 0) return [];
    const clearance = ROUTING_CLEARANCE_REM * viewport.remPx;
    return untrack(() =>
      projection.all().map((node) => ({
        left: node.position.x * viewport.remPx - clearance,
        top: node.position.y * viewport.remPx - clearance,
        right: (node.position.x + nodeWidth(node)) * viewport.remPx + clearance,
        bottom:
          (node.position.y + nodeHeight(node)) * viewport.remPx + clearance,
      })),
    );
  });

  let obstacleBuckets = $derived(
    routingObstacleBuckets(routingObstacles, viewport.remPx),
  );

  const socketIndex = (
    node: GraphNode,
    socketId: string,
    direction: GraphSocketDirection,
  ): number => {
    const sockets = displayNodeSockets(node, direction);
    return Math.max(
      0,
      sockets.findIndex((socket) => socket.id === socketId),
    );
  };

  const socketPoint = (
    reference: GraphSocketRef,
    direction: GraphSocketDirection,
  ): { x: number; y: number } | null => {
    const node = nodeById(reference.nodeId);
    if (!node) {
      return null;
    }
    if (
      node.collapsed === true &&
      allNodeSockets(node, direction).some(
        (socket) => socket.id === reference.socketId,
      )
    ) {
      return {
        x:
          (node.position.x +
            (direction === "output"
              ? nodeWidth(node) - HEADER_SOCKET_INSET_REM
              : HEADER_SOCKET_INSET_REM)) *
          viewport.remPx,
        y:
          (node.position.y + NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5) *
          viewport.remPx,
      };
    }
    if (node.socketPlacement === "header") {
      return {
        x:
          (node.position.x +
            (direction === "output"
              ? nodeWidth(node) - HEADER_SOCKET_INSET_REM
              : HEADER_SOCKET_INSET_REM)) *
          viewport.remPx,
        y:
          (node.position.y + NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5) *
          viewport.remPx,
      };
    }
    if (direction === "input" && node.headerInputs) {
      const headerIdx = node.headerInputs.findIndex(
        (s) => s.id === reference.socketId,
      );
      if (headerIdx >= 0) {
        return {
          x:
            (node.position.x +
              HEADER_SOCKET_INSET_REM +
              headerIdx * SOCKET_ROW_REM * 0.8) *
            viewport.remPx,
          y:
            (node.position.y + NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5) *
            viewport.remPx,
        };
      }
    }
    return {
      x:
        (node.position.x + (direction === "output" ? nodeWidth(node) : 0)) *
        viewport.remPx,
      y:
        (node.position.y +
          socketStart(node) +
          (socketIndex(node, reference.socketId, direction) + 0.5) *
            SOCKET_ROW_REM) *
        viewport.remPx,
    };
  };

  const localSocketPoint = (
    node: GraphNode,
    socketId: string,
    direction: GraphSocketDirection,
  ): { x: number; y: number } | null => {
    if (
      node.collapsed === true &&
      allNodeSockets(node, direction).some((socket) => socket.id === socketId)
    ) {
      return {
        x:
          direction === "output"
            ? nodeWidth(node) - HEADER_SOCKET_INSET_REM
            : HEADER_SOCKET_INSET_REM,
        y: NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5,
      };
    }
    if (node.socketPlacement === "header") {
      return {
        x:
          direction === "output"
            ? nodeWidth(node) - HEADER_SOCKET_INSET_REM
            : HEADER_SOCKET_INSET_REM,
        y: NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5,
      };
    }
    if (direction === "input" && node.headerInputs) {
      const headerIdx = node.headerInputs.findIndex(
        (socket) => socket.id === socketId,
      );
      if (headerIdx >= 0) {
        return {
          x: HEADER_SOCKET_INSET_REM + headerIdx * SOCKET_ROW_REM * 0.8,
          y: NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5,
        };
      }
    }
    return {
      x: direction === "output" ? nodeWidth(node) : 0,
      y:
        socketStart(node) +
        (socketIndex(node, socketId, direction) + 0.5) * SOCKET_ROW_REM,
    };
  };

  const wirePath = (
    start: { x: number; y: number },
    end: { x: number; y: number },
  ): string => {
    const horizontal = Math.abs(end.x - start.x);
    const control = clamp(
      horizontal * 0.5,
      viewport.remPx * 2.5,
      viewport.remPx * 12,
    );
    return `M ${start.x} ${start.y} C ${start.x + control} ${start.y}, ${end.x - control} ${end.y}, ${end.x} ${end.y}`;
  };

  const localWirePath = (
    start: { x: number; y: number },
    end: { x: number; y: number },
  ): string => {
    const horizontal = Math.abs(end.x - start.x);
    const control = clamp(horizontal * 0.5, 1.2, 6);
    return `M ${start.x} ${start.y} C ${start.x + control} ${start.y}, ${end.x - control} ${end.y}, ${end.x} ${end.y}`;
  };

  const nodeBypassPath = (
    node: GraphNode,
    bypass: GraphNodeBypassConnection,
  ): string | null => {
    const start = localSocketPoint(node, bypass.inputSocketId, "input");
    const end = localSocketPoint(node, bypass.outputSocketId, "output");
    return start && end ? localWirePath(start, end) : null;
  };

  const routedWirePath = (
    edge: GraphEdge,
    start: GraphNodePosition,
    end: GraphNodePosition,
  ): string | null => {
    const sourceNode = nodeById(edge.from.nodeId);
    const targetNode = nodeById(edge.to.nodeId);
    if (!sourceNode || !targetNode || sourceNode.id === targetNode.id) {
      return null;
    }
    const grid = ROUTING_GRID_REM * viewport.remPx;
    const clearance = ROUTING_CLEARANCE_REM * viewport.remPx;
    const routeStart = {
      x:
        Math.ceil(
          ((sourceNode.position.x + nodeWidth(sourceNode)) * viewport.remPx +
            clearance) /
            grid,
        ) *
          grid +
        grid,
      y: start.y,
    };
    const routeEnd = {
      x:
        Math.floor(
          (targetNode.position.x * viewport.remPx - clearance) / grid,
        ) *
          grid -
        grid,
      y: end.y,
    };
    return routeEdgeAroundObstacles(
      start,
      end,
      routeStart,
      routeEnd,
      routingObstacles,
      obstacleBuckets,
      viewport.remPx,
    );
  };

  const edgePath = (edge: GraphEdge): string | null => {
    const start = socketPoint(edge.from, "output");
    const end = socketPoint(edge.to, "input");
    if (!start || !end) {
      return null;
    }
    if (!props.routeEdgesAroundNodes || routingPreview) {
      return wirePath(start, end);
    }
    if (appliedRoutingKey !== routingGeometryKey) {
      routedPathCache.clear();
      appliedRoutingKey = routingGeometryKey;
    }
    const cacheKey = `${edge.id ?? ""}:${edge.from.nodeId}:${edge.from.socketId}:${edge.to.nodeId}:${edge.to.socketId}`;
    const cached = routedPathCache.get(cacheKey);
    if (cached) {
      return cached;
    }
    const path = routedWirePath(edge, start, end) ?? wirePath(start, end);
    if (routedPathCache.size >= 1024)
      routedPathCache.delete(routedPathCache.keys().next().value!);
    routedPathCache.set(cacheKey, path);
    return path;
  };

  const edgeUsesGradient = (edge: GraphEdge): boolean =>
    edge.color !== undefined &&
    edge.targetColor !== undefined &&
    edge.color !== edge.targetColor;

  const edgeGradientId = (edge: GraphEdge, index: number): string =>
    `edge-gradient-${(edge.id ?? `${edge.from.nodeId}-${edge.from.socketId}-${edge.to.nodeId}-${edge.to.socketId}-${index}`).replace(/[^a-zA-Z0-9_-]/g, "_")}`;

  const edgeStroke = (
    edge: GraphEdge,
    gradientId: string,
  ): string | undefined =>
    edgeUsesGradient(edge) ? `url(#${gradientId})` : edge.color;

  const draftPath = (): string | null => {
    if (!interaction.connectionDraft) {
      return null;
    }
    const start = socketPoint(interaction.connectionDraft.from, "output");
    if (!start) return null;

    if (
      interaction.connectionDraft.snapNodeId &&
      interaction.connectionDraft.snapSocketId
    ) {
      const end = socketPoint(
        {
          nodeId: interaction.connectionDraft.snapNodeId,
          socketId: interaction.connectionDraft.snapSocketId,
        },
        "input",
      );
      if (end) return wirePath(start, end);
    }

    return wirePath(start, {
      x: interaction.connectionDraft.endX,
      y: interaction.connectionDraft.endY,
    });
  };

  return {
    get nodes() {
      return nodes;
    },
    get effectiveNodes() {
      return effectiveNodes;
    },
    nodeById,
    sourceNode,
    queryNodes,
    get nodeCount() {
      frame;
      return projection.size;
    },
    presentationWork() {
      frame;
      return projection.statistics;
    },
    nodeWidth,
    nodeHeaderHeight,
    allNodeSockets,
    hasSocketConnection,
    socketExpanded,
    displaySockets,
    displayNodeSockets,
    connectionAllowed,
    socketDisabled,
    inputSocketContentVisible,
    socketDefaultColumnWidth,
    toggleSocketExpanded,
    firstCollapsedSocket,
    collapsedSocketLabel,
    collapsedSocketTitle,
    collapsedSocketConnected,
    collapsedSocketDraftTarget,
    minimumNodeHeight,
    nodeHeight,
    get visibleNodes() {
      return visibleNodes;
    },
    get visibleEdges() {
      return visibleEdges;
    },
    get connectedSockets() {
      return connectedSockets;
    },
    socketPoint,
    nodeBypassPath,
    edgePath,
    edgeUsesGradient,
    edgeGradientId,
    edgeStroke,
    draftPath,
  };
}

export type CanvasGeometry = ReturnType<typeof createCanvasGeometry>;
