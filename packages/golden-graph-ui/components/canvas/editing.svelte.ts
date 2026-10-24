import {
  TITLE_DOUBLE_CLICK_MS,
  TITLE_DOUBLE_CLICK_DISTANCE_PX,
  type CanvasProps,
} from "./contracts";
import type {
  GraphNode,
  GraphNodeMove,
  GraphNodePosition,
  GraphNodeResize,
  GraphNodeSize,
} from "../../types";

interface EditingScene {
  readonly nodes: readonly GraphNode[];
  readonly selectedIds: ReadonlySet<string>;
  updateSelection(nodeId: string, additive: boolean): void;
  startNodeDrag(event: PointerEvent, node: GraphNode): void;
}
export function createCanvasEditing(props: CanvasProps, scene: EditingScene) {
  let dragPositions = $state<Record<string, GraphNodePosition>>({});

  let optimisticPositions = $state<Record<string, GraphNodePosition>>({});

  let resizeSizes = $state<Record<string, GraphNodeSize>>({});

  let optimisticSizes = $state<Record<string, GraphNodeSize | null>>({});

  let optimisticLabels = $state<Record<string, string>>({});

  let optimisticCollapsed = $state<Record<string, boolean>>({});

  let optimisticEnabled = $state<Record<string, boolean>>({});

  let renamingNodeId: string | null = $state(null);

  let renameDraft = $state("");

  let renameInput: HTMLInputElement | null = $state(null);

  let focusedRenameNodeId: string | null = null;

  let lastTitlePointerDown: {
    nodeId: string;
    time: number;
    x: number;
    y: number;
  } | null = null;

  $effect(() => {
    let next = optimisticPositions;
    for (const node of scene.nodes) {
      const optimistic = next[node.id];
      if (
        optimistic &&
        Math.abs(node.position.x - optimistic.x) < 0.0001 &&
        Math.abs(node.position.y - optimistic.y) < 0.0001
      ) {
        if (next === optimisticPositions) {
          next = { ...optimisticPositions };
        }
        delete next[node.id];
      }
    }
    if (next !== optimisticPositions) {
      optimisticPositions = next;
    }
  });

  $effect(() => {
    let next = optimisticSizes;
    for (const node of scene.nodes) {
      const optimistic = next[node.id];
      if (
        optimistic !== undefined &&
        ((optimistic === null && node.size === undefined) ||
          (optimistic !== null &&
            node.size !== undefined &&
            Math.abs(node.size.width - optimistic.width) < 0.0001 &&
            Math.abs(node.size.height - optimistic.height) < 0.0001))
      ) {
        if (next === optimisticSizes) {
          next = { ...optimisticSizes };
        }
        delete next[node.id];
      }
    }
    if (next !== optimisticSizes) {
      optimisticSizes = next;
    }
  });

  $effect(() => {
    let next = optimisticLabels;
    const labelsById = new Map(
      scene.nodes.map((node) => [node.id, node.label]),
    );
    for (const [nodeId, label] of Object.entries(optimisticLabels)) {
      if (labelsById.get(nodeId) === label || !labelsById.has(nodeId)) {
        if (next === optimisticLabels) {
          next = { ...optimisticLabels };
        }
        delete next[nodeId];
      }
    }
    if (next !== optimisticLabels) {
      optimisticLabels = next;
    }
  });

  $effect(() => {
    let next = optimisticCollapsed;
    const collapsedById = new Map(
      scene.nodes.map((node) => [node.id, node.collapsed === true]),
    );
    for (const [nodeId, collapsed] of Object.entries(optimisticCollapsed)) {
      if (
        collapsedById.get(nodeId) === collapsed ||
        !collapsedById.has(nodeId)
      ) {
        if (next === optimisticCollapsed) {
          next = { ...optimisticCollapsed };
        }
        delete next[nodeId];
      }
    }
    if (next !== optimisticCollapsed) {
      optimisticCollapsed = next;
    }
  });

  $effect(() => {
    let next = optimisticEnabled;
    const enabledById = new Map(
      scene.nodes.map((node) => [node.id, node.enabled !== false]),
    );
    for (const [nodeId, enabled] of Object.entries(optimisticEnabled)) {
      if (enabledById.get(nodeId) === enabled || !enabledById.has(nodeId)) {
        if (next === optimisticEnabled) {
          next = { ...optimisticEnabled };
        }
        delete next[nodeId];
      }
    }
    if (next !== optimisticEnabled) {
      optimisticEnabled = next;
    }
  });

  $effect(() => {
    if (
      !renameInput ||
      !renamingNodeId ||
      focusedRenameNodeId === renamingNodeId
    ) {
      return;
    }
    focusedRenameNodeId = renamingNodeId;
    requestAnimationFrame(() => {
      renameInput?.focus();
      renameInput?.select();
    });
  });

  const clearOptimisticMoves = (moves: GraphNodeMove[]): void => {
    const next = { ...optimisticPositions };
    let changed = false;
    for (const move of moves) {
      const pending = next[move.nodeId];
      if (pending?.x === move.position.x && pending?.y === move.position.y) {
        delete next[move.nodeId];
        changed = true;
      }
    }
    if (changed) {
      optimisticPositions = next;
    }
  };

  const commitNodeMoves = (moves: GraphNodeMove[]): void => {
    optimisticPositions = {
      ...optimisticPositions,
      ...Object.fromEntries(moves.map((move) => [move.nodeId, move.position])),
    };
    try {
      const result = props.onNodesMove
        ? props.onNodesMove(moves)
        : Promise.all(
            moves.map((move) => props.onNodeMove?.(move.nodeId, move.position)),
          );
      void Promise.resolve(result).catch(() => clearOptimisticMoves(moves));
    } catch {
      clearOptimisticMoves(moves);
    }
  };

  const clearOptimisticResize = (resize: GraphNodeResize): void => {
    const pending = optimisticSizes[resize.nodeId];
    const matches =
      resize.mode === "automatic"
        ? pending === null
        : pending?.width === resize.size.width &&
          pending?.height === resize.size.height;
    if (matches) {
      const next = { ...optimisticSizes };
      delete next[resize.nodeId];
      optimisticSizes = next;
    }
  };

  const commitNodeResize = (resize: GraphNodeResize): void => {
    optimisticSizes = {
      ...optimisticSizes,
      [resize.nodeId]: resize.mode === "custom" ? resize.size : null,
    };
    try {
      void Promise.resolve(props.onNodeResize?.(resize)).catch(() =>
        clearOptimisticResize(resize),
      );
    } catch {
      clearOptimisticResize(resize);
    }
  };

  const resetNodeSize = (event: MouseEvent, node: GraphNode): void => {
    event.preventDefault();
    event.stopPropagation();
    if (!node.resizable) {
      return;
    }
    commitNodeResize({ nodeId: node.id, mode: "automatic" });
  };

  const clearOptimisticRename = (nodeId: string, label: string): void => {
    if (optimisticLabels[nodeId] !== label) {
      return;
    }
    const next = { ...optimisticLabels };
    delete next[nodeId];
    optimisticLabels = next;
  };

  const clearOptimisticCollapse = (
    nodeId: string,
    collapsed: boolean,
  ): void => {
    if (optimisticCollapsed[nodeId] !== collapsed) {
      return;
    }
    const next = { ...optimisticCollapsed };
    delete next[nodeId];
    optimisticCollapsed = next;
  };

  const clearOptimisticEnabled = (nodeId: string, enabled: boolean): void => {
    if (optimisticEnabled[nodeId] !== enabled) {
      return;
    }
    const next = { ...optimisticEnabled };
    delete next[nodeId];
    optimisticEnabled = next;
  };

  const toggleNodeCollapsed = (node: GraphNode): void => {
    const collapsed = node.collapsed !== true;
    optimisticCollapsed = { ...optimisticCollapsed, [node.id]: collapsed };
    try {
      void Promise.resolve(
        props.onNodeCollapsedChange?.(node.id, collapsed),
      ).catch(() => clearOptimisticCollapse(node.id, collapsed));
    } catch {
      clearOptimisticCollapse(node.id, collapsed);
    }
  };

  const canToggleNodeEnabled = (node: GraphNode): boolean =>
    props.onNodeEnabledChange !== undefined && node.canDisable === true;

  const toggleNodeEnabled = (event: MouseEvent, node: GraphNode): void => {
    event.preventDefault();
    event.stopPropagation();
    if (!canToggleNodeEnabled(node)) {
      return;
    }
    const enabled = node.enabled === false;
    optimisticEnabled = { ...optimisticEnabled, [node.id]: enabled };
    try {
      void Promise.resolve(props.onNodeEnabledChange?.(node.id, enabled)).catch(
        () => clearOptimisticEnabled(node.id, enabled),
      );
    } catch {
      clearOptimisticEnabled(node.id, enabled);
    }
  };

  const canRenameNode = (node: GraphNode): boolean =>
    props.onNodeRename !== undefined && node.canRename !== false;

  const isPurePrimaryClick = (event: MouseEvent | PointerEvent): boolean =>
    event.button === 0 &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey;

  const beginNodeRename = (event: MouseEvent, node: GraphNode): void => {
    if (!canRenameNode(node) || !isPurePrimaryClick(event)) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (!scene.selectedIds.has(node.id)) {
      scene.updateSelection(node.id, false);
    }
    renamingNodeId = node.id;
    renameDraft = node.label;
    focusedRenameNodeId = null;
  };

  const handleNodeTitlePointerDown = (
    event: PointerEvent,
    node: GraphNode,
  ): void => {
    if (!isPurePrimaryClick(event)) {
      lastTitlePointerDown = null;
      scene.startNodeDrag(event, node);
      return;
    }
    const elapsed = lastTitlePointerDown
      ? event.timeStamp - lastTitlePointerDown.time
      : Infinity;
    const distance = lastTitlePointerDown
      ? Math.hypot(
          event.clientX - lastTitlePointerDown.x,
          event.clientY - lastTitlePointerDown.y,
        )
      : Infinity;
    const doubleClick =
      lastTitlePointerDown?.nodeId === node.id &&
      elapsed >= 0 &&
      elapsed <= TITLE_DOUBLE_CLICK_MS &&
      distance <= TITLE_DOUBLE_CLICK_DISTANCE_PX;
    if (doubleClick && canRenameNode(node)) {
      lastTitlePointerDown = null;
      beginNodeRename(event, node);
      return;
    }
    lastTitlePointerDown = {
      nodeId: node.id,
      time: event.timeStamp,
      x: event.clientX,
      y: event.clientY,
    };
    scene.startNodeDrag(event, node);
  };

  const finishNodeRename = (): void => {
    const nodeId = renamingNodeId;
    if (!nodeId) {
      return;
    }
    const node = scene.nodes.find((candidate) => candidate.id === nodeId);
    const label = renameDraft.trim();
    renamingNodeId = null;
    renameInput = null;
    focusedRenameNodeId = null;
    if (!node || !label || label === node.label) {
      return;
    }
    optimisticLabels = { ...optimisticLabels, [nodeId]: label };
    try {
      void Promise.resolve(props.onNodeRename?.(nodeId, label)).catch(() =>
        clearOptimisticRename(nodeId, label),
      );
    } catch {
      clearOptimisticRename(nodeId, label);
    }
  };

  const cancelNodeRename = (): void => {
    renamingNodeId = null;
    renameInput = null;
    focusedRenameNodeId = null;
  };

  const handleRenameKeydown = (event: KeyboardEvent): void => {
    if (event.key === "Enter") {
      event.preventDefault();
      finishNodeRename();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancelNodeRename();
    }
  };

  return {
    get dragPositions() {
      return dragPositions;
    },
    set dragPositions(value: typeof dragPositions) {
      dragPositions = value;
    },
    get optimisticPositions() {
      return optimisticPositions;
    },
    get resizeSizes() {
      return resizeSizes;
    },
    set resizeSizes(value: typeof resizeSizes) {
      resizeSizes = value;
    },
    get optimisticSizes() {
      return optimisticSizes;
    },
    get optimisticLabels() {
      return optimisticLabels;
    },
    get optimisticCollapsed() {
      return optimisticCollapsed;
    },
    get optimisticEnabled() {
      return optimisticEnabled;
    },
    get renamingNodeId() {
      return renamingNodeId;
    },
    get renameDraft() {
      return renameDraft;
    },
    set renameDraft(value: typeof renameDraft) {
      renameDraft = value;
    },
    get renameInput() {
      return renameInput;
    },
    set renameInput(value: typeof renameInput) {
      renameInput = value;
    },
    commitNodeMoves,
    commitNodeResize,
    resetNodeSize,
    toggleNodeCollapsed,
    toggleNodeEnabled,
    canRenameNode,
    beginNodeRename,
    handleNodeTitlePointerDown,
    finishNodeRename,
    handleRenameKeydown,
  };
}

export type CanvasEditing = ReturnType<typeof createCanvasEditing>;
