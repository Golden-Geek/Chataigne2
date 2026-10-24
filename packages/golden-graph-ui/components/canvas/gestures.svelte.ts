import {
  type PanGesture,
  type NodeDragGesture,
  type NodeResizeGesture,
  type SelectionGesture,
  type ConnectionDraft,
  MIN_NODE_WIDTH_REM,
  TITLE_DOUBLE_CLICK_MS,
  TITLE_DOUBLE_CLICK_DISTANCE_PX,
  type CanvasProps,
} from "./contracts";
import type {
  GraphEdge,
  GraphNode,
  GraphNodeMove,
  GraphNodePosition,
  GraphSocket,
} from "../../types";
import type { CanvasViewport } from "./viewport.svelte";
import type { CanvasGeometry } from "./geometry.svelte";
import type { CanvasEditing } from "./editing.svelte";

export function createCanvasGestures(
  props: CanvasProps,
  viewport: CanvasViewport,
  geometry: CanvasGeometry,
  editing: CanvasEditing,
) {
  let panGesture = $state<PanGesture | null>(null);

  let nodeDragGesture = $state<NodeDragGesture | null>(null);

  let nodeResizeGesture = $state<NodeResizeGesture | null>(null);

  let selectionGesture = $state<SelectionGesture | null>(null);

  let connectionDraft = $state<ConnectionDraft | null>(null);

  let lastResizePointerDown: {
    nodeId: string;
    time: number;
    x: number;
    y: number;
  } | null = null;

  let selectedIds = $derived(new Set(props.selectedNodeIds));

  let selectedEdgeIdSet = $derived(new Set(props.selectedEdgeIds));

  let selectionBoxStyle = $derived.by(() => {
    if (!selectionGesture) {
      return "";
    }
    const left = Math.min(selectionGesture.startX, selectionGesture.currentX);
    const top = Math.min(selectionGesture.startY, selectionGesture.currentY);
    const width = Math.abs(selectionGesture.currentX - selectionGesture.startX);
    const height = Math.abs(
      selectionGesture.currentY - selectionGesture.startY,
    );
    return `left:${left}px;top:${top}px;width:${width}px;height:${height}px`;
  });

  const publishSelection = (nodeIds: string[], edgeIds: string[]): void => {
    props.onGraphSelectionChange?.(nodeIds, edgeIds);
  };

  const updateSelection = (nodeId: string, additive: boolean): void => {
    const next = additive ? new Set(selectedIds) : new Set<string>();
    if (additive && next.has(nodeId)) {
      next.delete(nodeId);
    } else {
      next.add(nodeId);
    }
    publishSelection([...next], additive ? [...selectedEdgeIdSet] : []);
  };

  const updateEdgeSelection = (edge: GraphEdge, additive: boolean): void => {
    if (!edge.id) {
      return;
    }
    const next = additive ? new Set(selectedEdgeIdSet) : new Set<string>();
    if (additive && next.has(edge.id)) {
      next.delete(edge.id);
    } else {
      next.add(edge.id);
    }
    publishSelection(additive ? [...selectedIds] : [], [...next]);
  };

  const selectEdge = (event: PointerEvent, edge: GraphEdge): void => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    viewport.container?.focus();
    updateEdgeSelection(edge, event.ctrlKey || event.metaKey || event.shiftKey);
  };

  const selectEdgeWithKeyboard = (
    event: KeyboardEvent,
    edge: GraphEdge,
  ): void => {
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    updateEdgeSelection(edge, event.ctrlKey || event.metaKey || event.shiftKey);
  };

  const localPointerPosition = (
    event: PointerEvent,
  ): { x: number; y: number } => {
    const bounds = viewport.container?.getBoundingClientRect();
    return {
      x: event.clientX - (bounds?.left ?? 0),
      y: event.clientY - (bounds?.top ?? 0),
    };
  };

  const finishSelectionGesture = (): void => {
    if (!selectionGesture) {
      return;
    }
    const left = Math.min(selectionGesture.startX, selectionGesture.currentX);
    const top = Math.min(selectionGesture.startY, selectionGesture.currentY);
    const right = Math.max(selectionGesture.startX, selectionGesture.currentX);
    const bottom = Math.max(selectionGesture.startY, selectionGesture.currentY);
    if (right - left < 3 && bottom - top < 3) {
      selectionGesture = null;
      return;
    }
    const next = new Set(selectionGesture.initialSelectedNodeIds);
    const scale = viewport.remPx * viewport.camera.zoom;
    for (const node of geometry.queryNodes({
      left: (left - viewport.camera.x) / scale,
      top: (top - viewport.camera.y) / scale,
      right: (right - viewport.camera.x) / scale,
      bottom: (bottom - viewport.camera.y) / scale,
    }))
      next.add(node.id);
    publishSelection([...next], [...selectedEdgeIdSet]);
    selectionGesture = null;
  };

  const startPan = (event: PointerEvent): void => {
    if (event.button !== 0 && event.button !== 1) {
      return;
    }
    viewport.container?.focus();
    if (event.button === 0 && event.shiftKey) {
      const pointer = localPointerPosition(event);
      selectionGesture = {
        pointerId: event.pointerId,
        startX: pointer.x,
        startY: pointer.y,
        currentX: pointer.x,
        currentY: pointer.y,
        initialSelectedNodeIds: [...selectedIds],
      };
      viewport.container?.setPointerCapture(event.pointerId);
      return;
    }
    panGesture = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      cameraX: viewport.camera.x,
      cameraY: viewport.camera.y,
      moved: false,
      clearSelectionOnClick: event.button === 0,
    };
    viewport.container?.setPointerCapture(event.pointerId);
  };

  const selectNodeBody = (event: PointerEvent, node: GraphNode): void => {
    if (event.button !== 0) {
      return;
    }
    event.stopPropagation();
    viewport.container?.focus();
    const target = event.target;
    if (
      event.altKey &&
      target instanceof Element &&
      target.closest(".node-header")
    ) {
      event.preventDefault();
      editing.toggleNodeCollapsed(node);
      return;
    }
    if (
      target instanceof Element &&
      target.closest(
        "button, input, textarea, select, a, [data-no-node-select]",
      )
    ) {
      return;
    }
    if (event.ctrlKey || event.metaKey || event.shiftKey) {
      updateSelection(node.id, true);
    } else if (!selectedIds.has(node.id)) {
      updateSelection(node.id, false);
    }
  };

  const startNodeDrag = (event: PointerEvent, node: GraphNode): void => {
    if (event.button !== 0) {
      return;
    }
    event.stopPropagation();
    viewport.container?.focus();
    const target = event.target;
    if (
      event.altKey &&
      target instanceof Element &&
      target.closest(".node-header")
    ) {
      event.preventDefault();
      editing.toggleNodeCollapsed(node);
      return;
    }
    const additive = event.ctrlKey || event.metaKey || event.shiftKey;
    let dragIds: Set<string>;
    if (additive) {
      const next = new Set(selectedIds);
      if (next.has(node.id)) {
        next.delete(node.id);
      } else {
        next.add(node.id);
      }
      publishSelection([...next], additive ? [...selectedEdgeIdSet] : []);
      if (!next.has(node.id)) {
        return;
      }
      dragIds = next;
    } else if (selectedIds.has(node.id)) {
      dragIds = new Set(selectedIds);
    } else {
      updateSelection(node.id, false);
      dragIds = new Set([node.id]);
    }
    const nodePositions: Record<string, GraphNodePosition> = {};
    for (const dragId of dragIds) {
      const dragNode = geometry.nodeById(dragId);
      if (dragNode) {
        nodePositions[dragId] = dragNode.position;
      }
    }
    nodeDragGesture = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      nodePositions,
    };
    viewport.container?.setPointerCapture(event.pointerId);
  };

  const startNodeResize = (event: PointerEvent, node: GraphNode): void => {
    if (event.button !== 0 || !node.resizable) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    viewport.container?.focus();
    const now = Date.now();
    const previousPointerDown = lastResizePointerDown;
    lastResizePointerDown = {
      nodeId: node.id,
      time: now,
      x: event.clientX,
      y: event.clientY,
    };
    if (
      previousPointerDown?.nodeId === node.id &&
      now - previousPointerDown.time <= TITLE_DOUBLE_CLICK_MS &&
      Math.hypot(
        event.clientX - previousPointerDown.x,
        event.clientY - previousPointerDown.y,
      ) <= TITLE_DOUBLE_CLICK_DISTANCE_PX
    ) {
      lastResizePointerDown = null;
      if (!selectedIds.has(node.id)) {
        updateSelection(node.id, false);
      }
      editing.commitNodeResize({ nodeId: node.id, mode: "automatic" });
      return;
    }
    if (!selectedIds.has(node.id)) {
      updateSelection(node.id, false);
    }
    nodeResizeGesture = {
      pointerId: event.pointerId,
      nodeId: node.id,
      startX: event.clientX,
      startY: event.clientY,
      startSize: {
        width: geometry.nodeWidth(node),
        height: geometry.nodeHeight(node),
      },
    };
    viewport.container?.setPointerCapture(event.pointerId);
  };

  const startConnection = (
    event: PointerEvent,
    nodeId: string,
    socket: GraphSocket,
  ): void => {
    const node = geometry.nodeById(nodeId);
    if (
      event.button !== 0 ||
      !node ||
      geometry.socketDisabled(node, socket, "output")
    ) {
      return;
    }
    event.stopPropagation();
    const point = geometry.socketPoint(
      { nodeId, socketId: socket.id },
      "output",
    );
    if (!point) {
      return;
    }
    connectionDraft = {
      pointerId: event.pointerId,
      from: { nodeId, socketId: socket.id },
      endX: point.x,
      endY: point.y,
    };
    viewport.container?.setPointerCapture(event.pointerId);
  };

  const completeConnection = (nodeId: string, socket: GraphSocket): boolean => {
    const node = geometry.nodeById(nodeId);
    if (
      !connectionDraft ||
      !node ||
      geometry.socketDisabled(node, socket, "input")
    ) {
      return false;
    }
    const connection = {
      from: connectionDraft.from,
      to: { nodeId, socketId: socket.id },
    };
    if (!geometry.connectionAllowed(connection)) {
      return false;
    }
    props.onConnect?.(connection);
    connectionDraft = null;
    return true;
  };

  const finishConnection = (
    event: PointerEvent,
    nodeId: string,
    socket: GraphSocket,
  ): void => {
    if (completeConnection(nodeId, socket)) {
      event.stopPropagation();
    }
  };

  const findInputSocket = (
    node: GraphNode | undefined,
    socketId: string,
  ): GraphSocket | undefined =>
    node
      ? geometry
          .allNodeSockets(node, "input")
          .find((socket) => socket.id === socketId)
      : undefined;

  const finishConnectionAtPointer = (event: PointerEvent): void => {
    if (connectionDraft?.snapNodeId && connectionDraft?.snapSocketId) {
      const socket = findInputSocket(
        geometry.nodeById(connectionDraft.snapNodeId),
        connectionDraft.snapSocketId,
      );
      if (socket) {
        completeConnection(connectionDraft.snapNodeId, socket);
        return;
      }
    }

    const target = document.elementFromPoint(event.clientX, event.clientY);
    const inputSocket =
      target instanceof Element
        ? target.closest<HTMLButtonElement>(
            ".socket.input[data-node-id][data-socket-id]",
          )
        : null;
    const nodeId = inputSocket?.dataset.nodeId;
    const socketId = inputSocket?.dataset.socketId;
    if (!nodeId || !socketId) {
      return;
    }
    const socket = findInputSocket(geometry.nodeById(nodeId), socketId);
    if (socket) {
      completeConnection(nodeId, socket);
    }
  };

  const handlePointerMove = (event: PointerEvent): void => {
    const hasActiveGesture =
      selectionGesture?.pointerId === event.pointerId ||
      panGesture?.pointerId === event.pointerId ||
      nodeDragGesture?.pointerId === event.pointerId ||
      nodeResizeGesture?.pointerId === event.pointerId ||
      connectionDraft?.pointerId === event.pointerId;
    if (!hasActiveGesture) {
      viewport.updateScrollHoverTarget(event);
    }
    if (selectionGesture?.pointerId === event.pointerId) {
      const pointer = localPointerPosition(event);
      selectionGesture = {
        ...selectionGesture,
        currentX: pointer.x,
        currentY: pointer.y,
      };
      return;
    }
    if (panGesture?.pointerId === event.pointerId) {
      const deltaX = event.clientX - panGesture.startX;
      const deltaY = event.clientY - panGesture.startY;
      if (!panGesture.moved && Math.hypot(deltaX, deltaY) >= 3) {
        panGesture = { ...panGesture, moved: true };
      }
      viewport.applyCamera({
        ...viewport.camera,
        x: panGesture.cameraX + deltaX,
        y: panGesture.cameraY + deltaY,
      });
      return;
    }
    if (nodeDragGesture?.pointerId === event.pointerId) {
      const deltaX =
        (event.clientX - nodeDragGesture.startX) /
        viewport.camera.zoom /
        viewport.remPx;
      const deltaY =
        (event.clientY - nodeDragGesture.startY) /
        viewport.camera.zoom /
        viewport.remPx;
      editing.previewMoves(
        Object.entries(nodeDragGesture.nodePositions).map(
          ([nodeId, position]) => ({
            nodeId,
            position: { x: position.x + deltaX, y: position.y + deltaY },
          }),
        ),
      );
      return;
    }
    if (nodeResizeGesture?.pointerId === event.pointerId) {
      const node = geometry.nodeById(nodeResizeGesture.nodeId);
      if (!node) {
        return;
      }
      const deltaX =
        (event.clientX - nodeResizeGesture.startX) /
        viewport.camera.zoom /
        viewport.remPx;
      const deltaY =
        (event.clientY - nodeResizeGesture.startY) /
        viewport.camera.zoom /
        viewport.remPx;
      editing.previewResize(node.id, {
        width: Math.max(
          MIN_NODE_WIDTH_REM,
          nodeResizeGesture.startSize.width + deltaX,
        ),
        height: Math.max(
          geometry.minimumNodeHeight(node),
          nodeResizeGesture.startSize.height + deltaY,
        ),
      });
      return;
    }
    if (connectionDraft?.pointerId === event.pointerId) {
      const world = viewport.clientToWorldPx(event.clientX, event.clientY);

      let bestSnapDist = viewport.remPx * 4; // max snap distance
      let bestNodeId: string | undefined = undefined;
      let bestSocketId: string | undefined = undefined;

      for (const node of geometry.queryNodes({
        left: world.x / viewport.remPx - 4,
        top: world.y / viewport.remPx - 4,
        right: world.x / viewport.remPx + 4,
        bottom: world.y / viewport.remPx + 4,
      })) {
        if (node.id === connectionDraft.from.nodeId) continue;

        for (const socket of geometry.displayNodeSockets(node, "input")) {
          if (geometry.socketDisabled(node, socket, "input")) continue;
          const pt = geometry.socketPoint(
            { nodeId: node.id, socketId: socket.id },
            "input",
          );
          if (!pt) continue;

          const dist = Math.hypot(world.x - pt.x, world.y - pt.y);
          if (dist < bestSnapDist) {
            bestSnapDist = dist;
            bestNodeId = node.id;
            bestSocketId = socket.id;
          }
        }

        // Snap to node header if close to the node bounds and it has exactly one compatible input
        if (
          node.inputs.length === 1 &&
          !geometry.socketDisabled(node, node.inputs[0], "input")
        ) {
          const left = node.position.x * viewport.remPx;
          const top = node.position.y * viewport.remPx;
          const right = left + geometry.nodeWidth(node) * viewport.remPx;
          const bottom = top + geometry.nodeHeight(node) * viewport.remPx;

          // If inside node bounds or slightly expanded bounds
          const margin = viewport.remPx * 1;
          if (
            world.x >= left - margin &&
            world.x <= right + margin &&
            world.y >= top - margin &&
            world.y <= bottom + margin
          ) {
            // Only snap to the whole node if it's better than our current best distance
            // Being inside the node bounds is considered distance 0
            if (bestSnapDist > 0) {
              bestSnapDist = 0; // Prioritize this since we are directly over the node
              bestNodeId = node.id;
              bestSocketId = node.inputs[0].id;
            }
          }
        }
      }

      connectionDraft = {
        ...connectionDraft,
        endX: world.x,
        endY: world.y,
        snapNodeId: bestNodeId,
        snapSocketId: bestSocketId,
      };
    }
  };

  const handlePointerEnd = (event: PointerEvent): void => {
    if (selectionGesture?.pointerId === event.pointerId) {
      finishSelectionGesture();
    }
    if (panGesture?.pointerId === event.pointerId) {
      if (!panGesture.moved && panGesture.clearSelectionOnClick) {
        publishSelection([], []);
      }
      panGesture = null;
    }
    if (nodeDragGesture?.pointerId === event.pointerId) {
      const moves = Object.entries(editing.dragPositions).map(
        ([nodeId, position]): GraphNodeMove => ({
          nodeId,
          position,
        }),
      );
      if (moves.length > 0) {
        editing.commitNodeMoves(moves);
      }
      editing.dragPositions = {};
      nodeDragGesture = null;
    }
    if (nodeResizeGesture?.pointerId === event.pointerId) {
      const size = editing.resizeSizes[nodeResizeGesture.nodeId];
      if (size) {
        editing.commitNodeResize({
          nodeId: nodeResizeGesture.nodeId,
          mode: "custom",
          size,
        });
      }
      editing.resizeSizes = {};
      nodeResizeGesture = null;
    }
    if (connectionDraft?.pointerId === event.pointerId) {
      finishConnectionAtPointer(event);
      connectionDraft = null;
    }
    if (viewport.container?.hasPointerCapture(event.pointerId)) {
      viewport.container.releasePointerCapture(event.pointerId);
    }
  };

  const handleContextMenu = (event: MouseEvent): void => {
    const targetElement = event.target instanceof Element ? event.target : null;
    const nodeTarget = targetElement?.closest<HTMLElement>("[data-node-id]");
    if (nodeTarget) {
      event.preventDefault();
      event.stopPropagation();
      const nodeId = nodeTarget.dataset.nodeId;
      if (nodeId) {
        props.onNodeContextMenu?.(event, nodeId);
      }
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (!props.onBackgroundContextMenu || targetElement?.closest(".toolbar")) {
      return;
    }
    props.onBackgroundContextMenu?.(
      event,
      viewport.clientToWorld(event.clientX, event.clientY),
    );
  };

  const handleDoubleClick = (event: MouseEvent): void => {
    const targetElement = event.target instanceof Element ? event.target : null;
    if (
      !props.onBackgroundDoubleClick ||
      targetElement?.closest(
        '.toolbar, [data-node-id], button, input, select, textarea, a, [role="button"]',
      )
    ) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    props.onBackgroundDoubleClick(
      event,
      viewport.clientToWorld(event.clientX, event.clientY),
    );
  };

  const handleKeydown = (event: KeyboardEvent): void => {
    if (
      event.defaultPrevented ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey
    ) {
      return;
    }
    const target = event.target;
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLSelectElement ||
      target instanceof HTMLTextAreaElement ||
      (target instanceof HTMLElement && target.isContentEditable)
    ) {
      return;
    }
    if (event.key.toLowerCase() === "f") {
      event.preventDefault();
      event.stopPropagation();
      viewport.frameSelection();
    } else if (event.key.toLowerCase() === "h") {
      event.preventDefault();
      event.stopPropagation();
      viewport.home();
    } else if (
      event.key === " " &&
      props.onCreateRequest &&
      viewport.container
    ) {
      event.preventDefault();
      event.stopPropagation();
      const bounds = viewport.container.getBoundingClientRect();
      props.onCreateRequest({
        position: viewport.viewportCenter(),
        clientX: bounds.left + bounds.width * 0.5,
        clientY: bounds.top + bounds.height * 0.5,
      });
    }
  };

  return {
    get panGesture() {
      return panGesture;
    },
    get nodeDragGesture() {
      return nodeDragGesture;
    },
    get nodeResizeGesture() {
      return nodeResizeGesture;
    },
    get selectionGesture() {
      return selectionGesture;
    },
    get connectionDraft() {
      return connectionDraft;
    },
    get selectedIds() {
      return selectedIds;
    },
    get selectedEdgeIdSet() {
      return selectedEdgeIdSet;
    },
    get selectionBoxStyle() {
      return selectionBoxStyle;
    },
    updateSelection,
    selectEdge,
    selectEdgeWithKeyboard,
    startPan,
    selectNodeBody,
    startNodeDrag,
    startNodeResize,
    startConnection,
    finishConnection,
    handlePointerMove,
    handlePointerEnd,
    handleContextMenu,
    handleDoubleClick,
    handleKeydown,
  };
}

export type CanvasGestures = ReturnType<typeof createCanvasGestures>;
