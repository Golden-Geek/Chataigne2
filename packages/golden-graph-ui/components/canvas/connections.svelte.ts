import { onDestroy, untrack } from "svelte";
import type { GraphConnectionRequest } from "../../types";
import type { CanvasProps } from "./contracts";

/** Pointer and keyboard requests share dispatch, failure feedback and owner fencing. */
export function createCanvasConnectionRequests(props: CanvasProps) {
  let message = $state("");
  let sourceIdentity = $derived(props.graphDocument.source);
  let interactionIdentity = $derived(props.interactionScope);
  let scope = $derived({ owner: interactionIdentity, source: sourceIdentity });
  let applied: unknown;
  let generation = 0;
  let disposed = false;
  $effect.pre(() => {
    const owner = scope;
    untrack(() => {
      if (owner !== applied) {
        applied = owner;
        generation++;
        message = "";
      }
    });
  });
  const request = (connection: GraphConnectionRequest): boolean => {
    if (
      disposed ||
      !props.onConnect ||
      !(props.canConnect?.(connection) ?? true)
    )
      return false;
    const owner = scope,
      token = ++generation;
    try {
      void Promise.resolve(props.onConnect(connection)).catch(() => {
        if (!disposed && owner === scope && token === generation)
          message = "Connection request failed.";
      });
    } catch {
      message = "Connection request failed.";
      return false;
    }
    message = "Connection requested.";
    return true;
  };
  onDestroy(() => {
    disposed = true;
    generation++;
  });
  return {
    request,
    clearMessage: () => {
      generation++;
      message = "";
    },
    get message() {
      return message;
    },
  };
}
export type CanvasConnectionRequests = ReturnType<
  typeof createCanvasConnectionRequests
>;
