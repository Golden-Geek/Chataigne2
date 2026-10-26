import type { GraphConnectionRequest } from "../../types";
import type { CanvasProps } from "./contracts";
import type { CanvasMutationFeedback } from "./feedback.svelte";

/** Pointer and keyboard requests share dispatch, failure feedback and owner fencing. */
export function createCanvasConnectionRequests(
  props: CanvasProps,
  feedback: CanvasMutationFeedback,
) {
  const request = (connection: GraphConnectionRequest): boolean => {
    if (!props.onConnect || !(props.canConnect?.(connection) ?? true))
      return false;
    return feedback.dispatch(
      "Connection requested.",
      "Connection request failed.",
      () => props.onConnect!(connection),
    );
  };
  return {
    request,
    clearMessage: feedback.clear,
    get message() {
      return feedback.message;
    },
  };
}
export type CanvasConnectionRequests = ReturnType<
  typeof createCanvasConnectionRequests
>;
