import { onDestroy, untrack } from "svelte";
import {
  chooseGraphEndpoint,
  graphKeyboardMoves,
  type GraphKeyboardEndpoint,
} from "../../keyboard";
import type { GraphSocketDirection, GraphSocketRef } from "../../types";
import type { CanvasProps } from "./contracts";
import type { CanvasEditing } from "./editing.svelte";
import type { CanvasGeometry } from "./geometry.svelte";
import type { CanvasGestures } from "./gestures.svelte";
import type { CanvasConnectionRequests } from "./connections.svelte";

export function createCanvasKeyboard(
  props: CanvasProps,
  geometry: CanvasGeometry,
  editing: CanvasEditing,
  gestures: CanvasGestures,
  connections: CanvasConnectionRequests,
) {
  let endpoint = $state<GraphKeyboardEndpoint | null>(null);
  let message = $state("");
  let sourceIdentity = $derived(props.graphDocument.source);
  let interactionIdentity = $derived(props.interactionScope);
  let topologyRevision = $derived(props.graphDocument.revision.topology);
  let scope = $derived({
    owner: interactionIdentity,
    source: sourceIdentity,
    topology: topologyRevision,
  });
  let applied: unknown;
  let disposed = false;
  const cancel = () => {
    const active = endpoint !== null;
    endpoint = null;
    if (active) {
      connections.clearMessage();
      message = "Connection cancelled.";
    }
    return active;
  };
  $effect.pre(() => {
    const next = scope,
      connect = props.onConnect,
      pointer = gestures.connectionDraft;
    untrack(() => {
      if (next !== applied) {
        const active = endpoint !== null;
        endpoint = null;
        if (active) connections.clearMessage();
        message = active
          ? "Connection cancelled because the graph changed."
          : "";
      } else if (!connect || pointer) cancel();
      applied = next;
    });
  });
  const socket = (
    reference: GraphSocketRef,
    direction: GraphSocketDirection,
  ) => {
    const node = geometry.nodeById(reference.nodeId);
    const value =
      node &&
      geometry
        .allNodeSockets(node, direction)
        .find((value) => value.id === reference.socketId);
    return node && value && !geometry.socketDisabled(node, value, direction)
      ? { node, value }
      : null;
  };
  const activateSocket = (
    nodeId: string,
    socketId: string,
    direction: GraphSocketDirection,
  ): boolean => {
    if (disposed || !props.onConnect || gestures.connectionDraft) return false;
    const next = { nodeId, socketId, direction };
    if (!socket(next, direction)) return false;
    if (endpoint && !socket(endpoint, endpoint.direction)) cancel();
    const choice = chooseGraphEndpoint(endpoint, next);
    if (!choice.connection) {
      connections.clearMessage();
      endpoint = choice.endpoint;
      message = endpoint
        ? `Choose an ${direction === "output" ? "input" : "output"} socket to connect.`
        : "Connection cancelled.";
      return true;
    }
    if (!geometry.connectionAllowed(choice.connection)) {
      message = "These sockets cannot be connected.";
      return false;
    }
    if (!connections.request(choice.connection)) return false;
    endpoint = null;
    message = "Connection requested.";
    return true;
  };
  const move = (key: string, large = false): boolean => {
    if (disposed || (!props.onNodesMove && !props.onNodeMove)) return false;
    if (
      gestures.nodeDragGesture ||
      gestures.nodeResizeGesture ||
      gestures.panGesture ||
      gestures.selectionGesture
    )
      return false;
    const moves = graphKeyboardMoves(
      props.selectedNodeIds,
      geometry.nodeById,
      key,
      large,
    );
    if (!moves.length) return false;
    editing.commitNodeMoves(moves);
    connections.clearMessage();
    message = `Move requested for ${moves.length} selected ${moves.length === 1 ? "node" : "nodes"}.`;
    return true;
  };
  const selectNode = (id: string, additive = false) => {
    if (disposed || !geometry.nodeById(id) || !props.onGraphSelectionChange)
      return false;
    gestures.updateSelection(id, additive);
    return true;
  };
  const handleNodeKeydown = (event: KeyboardEvent, id: string) => {
    if (event.defaultPrevented || (event.key !== "Enter" && event.key !== " "))
      return;
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const direction = target.dataset.socketDirection,
      socketId = target.dataset.socketId;
    if (socketId && (direction === "input" || direction === "output")) {
      event.preventDefault();
      event.stopPropagation();
      activateSocket(id, socketId, direction);
    } else if (target.classList.contains("node-title-label")) {
      event.preventDefault();
      event.stopPropagation();
      selectNode(id, event.ctrlKey || event.metaKey || event.shiftKey);
    }
  };
  const handleKeydown = (event: KeyboardEvent): boolean => {
    if (
      event.defaultPrevented ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey
    )
      return false;
    if (event.key === "Escape") {
      const handled = cancel() || gestures.cancelConnection();
      if (handled) {
        event.preventDefault();
        event.stopPropagation();
      }
      return handled;
    }
    const target = event.target;
    if (
      target instanceof HTMLElement &&
      (target.isContentEditable ||
        target.closest(
          "input, select, textarea, button:not(.node-title-label), a, [data-no-node-select]",
        ))
    )
      return false;
    const handled =
      event.key === "Escape" ? cancel() : move(event.key, event.shiftKey);
    if (handled) {
      event.preventDefault();
      event.stopPropagation();
    }
    return handled;
  };
  onDestroy(() => {
    disposed = true;
    endpoint = null;
  });
  const isEndpoint = (
    id: string,
    socketId: string,
    direction: GraphSocketDirection,
  ) =>
    endpoint?.nodeId === id &&
    endpoint.socketId === socketId &&
    endpoint.direction === direction;
  return {
    get endpoint() {
      return endpoint;
    },
    get message() {
      return message;
    },
    move,
    selectNode,
    activateSocket,
    cancel,
    isEndpoint,
    handleNodeKeydown,
    handleKeydown,
  };
}
export type CanvasKeyboard = ReturnType<typeof createCanvasKeyboard>;
