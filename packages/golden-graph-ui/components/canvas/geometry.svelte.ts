import {
  type ConnectionDraft,
  DEFAULT_NODE_WIDTH_REM,
  MIN_NODE_WIDTH_REM,
  NODE_BORDER_REM,
  NODE_HEADER_REM,
  SOCKET_ROW_REM,
  HEADER_SOCKET_INSET_REM,
  clamp,
  type CanvasProps,
} from "./contracts";
import type {
  GraphConnectionRequest,
  GraphNode,
  GraphNodeBypassConnection,
  GraphNodePosition,
  GraphNodeSize,
  GraphSocket,
  GraphSocketDirection,
  GraphSocketRef,
} from "../../types";
import { onDestroy, untrack } from "svelte";
import { RetainedGraphPresentation } from "../../retained-presentation";
import type { SpatialBounds } from "../../spatial-index";
import { viewportWorldBounds } from "../../presentation-projection";
import { RetainedEdgePresentation } from "../../retained-edges";
import type { GraphNodePresentationChanges } from "../../presentation-projection";
import type { CanvasViewport } from "./viewport.svelte";
import { GraphSocketConnections } from "../../socket-connections";

interface GeometryInteraction extends Readonly<GraphNodePresentationChanges> {
  readonly connectionDraft: ConnectionDraft | null;
}
export function createCanvasGeometry(
  props: CanvasProps,
  viewport: CanvasViewport,
  interaction: GeometryInteraction,
) {
  let disposed = false;
  onDestroy(() => {
    disposed = true;
  });
  let nodes = $derived(props.graphDocument.nodes);

  let edges = $derived(props.graphDocument.edges);

  const expandedSockets = new Map<string, boolean>();
  let expansionRevision = $state(0);
  const expandedNodeIds = new Set<string>();
  const layoutDirtyIds = new Set<string>();
  let sourceIdentity = $derived(props.graphDocument.source);
  let ownerIdentity = $derived(props.interactionScope);
  let expansionScope = $derived({
    source: sourceIdentity,
    owner: ownerIdentity,
  });
  let appliedExpansionScope: unknown;
  $effect.pre(() => {
    const scope = expansionScope;
    untrack(() => {
      if (scope === appliedExpansionScope) return;
      appliedExpansionScope = scope;
      for (const id of expandedNodeIds) layoutDirtyIds.add(id);
      expandedNodeIds.clear();
      expandedSockets.clear();
      expansionRevision++;
    });
  });
  const projection = new RetainedGraphPresentation();
  const routes = new RetainedEdgePresentation();
  let routeRevision = $state(0);
  let cachedTopologySource: object | null = null;
  let cachedTopologyRevision = -1;
  let cachedTopology = new GraphSocketConnections([]);
  let topologyView = $derived.by(() => {
    const document = props.graphDocument;
    if (
      document.source !== cachedTopologySource ||
      document.revision.topology !== cachedTopologyRevision
    ) {
      cachedTopologySource = document.source;
      cachedTopologyRevision = document.revision.topology;
      cachedTopology = new GraphSocketConnections(document.edges);
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
    expansionRevision;
    topologyView;
    return untrack(() => {
      const delta = projection.synchronize(
        document,
        changes,
        nodeWidth,
        nodeHeight,
        layoutDirtyIds,
      );
      routes.synchronize(document, delta, (reference, direction) => {
        const node = projection.nodesById.get(reference.nodeId);
        if (!node) return null;
        const point = localSocketPoint(node, reference.socketId, direction);
        return point
          ? { x: node.position.x + point.x, y: node.position.y + point.y }
          : null;
      });
      layoutDirtyIds.clear();
      return { geometry: projection.geometryRevision };
    });
  });
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

  const socketExpansionKey = (
    node: GraphNode,
    socket: GraphSocket,
    direction: GraphSocketDirection,
  ): string => JSON.stringify([node.id, direction, socket.id]);

  const hasSocketConnection = (
    node: GraphNode,
    socketId: string,
    direction: GraphSocketDirection,
  ): boolean => socketConnections.has(node.id, socketId, direction);

  const socketHasConnectedChild = (
    node: GraphNode,
    socket: GraphSocket,
    direction: GraphSocketDirection,
  ): boolean =>
    (socket.children ?? []).some(
      (child) =>
        hasSocketConnection(node, child.id, direction) ||
        socketHasConnectedChild(node, child, direction),
    );

  const socketExpanded = (
    node: GraphNode,
    socket: GraphSocket,
    direction: GraphSocketDirection,
  ): boolean => {
    expansionRevision;
    return (
      (socket.children?.length ?? 0) > 0 &&
      (socketHasConnectedChild(node, socket, direction) ||
        expandedSockets.get(socketExpansionKey(node, socket, direction)) ===
          true)
    );
  };

  const displaySockets = (
    node: GraphNode,
    direction: GraphSocketDirection,
    sockets: readonly GraphSocket[],
  ): GraphSocket[] =>
    sockets.flatMap((socket) =>
      socketExpanded(node, socket, direction)
        ? [socket, ...displaySockets(node, direction, socket.children ?? [])]
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
    if (!props.onConnect || socket.compatible === false) return true;
    if (direction === "output") return false;
    let parentId = socket.parentId;
    const ancestors = new Set<string>();
    while (parentId) {
      if (
        ancestors.has(parentId) ||
        hasSocketConnection(node, parentId, direction)
      )
        return true;
      ancestors.add(parentId);
      parentId = allNodeSockets(node, direction).find(
        (value) => value.id === parentId,
      )?.parentId;
    }
    if (socketHasConnectedChild(node, socket, direction)) return true;
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
    !hasSocketConnection(node, socket.id, "input") &&
    !socketHasConnectedChild(node, socket, "input");

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
    nodeId: string,
    socketId: string,
    direction: GraphSocketDirection,
  ): boolean => {
    if (disposed) return false;
    const node = nodeById(nodeId);
    const socket =
      node &&
      allNodeSockets(node, direction).find((value) => value.id === socketId);
    if (
      !node ||
      !socket?.children?.length ||
      socketHasConnectedChild(node, socket, direction)
    )
      return false;
    const key = socketExpansionKey(node, socket, direction);
    layoutDirtyIds.add(node.id);
    expandedNodeIds.add(node.id);
    expandedSockets.set(key, !socketExpanded(node, socket, direction));
    expansionRevision++;
    return true;
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

  const activatableSockets = (
    node: GraphNode,
    direction: GraphSocketDirection,
  ): GraphSocket[] => {
    if (node.collapsed) {
      const first = firstCollapsedSocket(node, direction);
      return first ? [first] : [];
    }
    if (node.socketPlacement === "header")
      return direction === "input" ? node.inputs : node.outputs;
    return displayNodeSockets(node, direction);
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
      hasSocketConnection(node, socket.id, direction),
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

  let socketConnections = $derived(topologyView);
  let routingPreview = $derived(
    Object.keys(interaction.dragPositions).length > 0 ||
      Object.keys(interaction.resizeSizes).length > 0,
  );
  let visibleEdges = $derived.by(() => {
    frame;
    routeRevision;
    return routes.visible(
      viewportWorldBounds(
        viewport.camera,
        viewport.viewportWidth,
        viewport.viewportHeight,
        viewport.remPx,
        8,
      ),
      props.routeEdgesAroundNodes,
      routingPreview,
    );
  });
  $effect(() => {
    visibleEdges;
    if (!routes.pending) return;
    const handle = requestAnimationFrame(() => {
      routes.processPending();
      routeRevision += 1;
    });
    return () => cancelAnimationFrame(handle);
  });

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
  ): GraphNodePosition | null => {
    const node = nodeById(reference.nodeId);
    if (!node) return null;
    const point = localSocketPoint(node, reference.socketId, direction);
    return point
      ? {
          x: (node.position.x + point.x) * viewport.remPx,
          y: (node.position.y + point.y) * viewport.remPx,
        }
      : null;
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
    routingWork() {
      frame;
      return routes.statistics;
    },
    presentationWork() {
      frame;
      return projection.statistics;
    },
    nodeWidth,
    nodeHeaderHeight,
    allNodeSockets,
    activatableSockets,
    hasSocketConnection,
    socketExpanded,
    socketHasConnectedChild,
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
    socketPoint,
    nodeBypassPath,
    draftPath,
  };
}

export type CanvasGeometry = ReturnType<typeof createCanvasGeometry>;
