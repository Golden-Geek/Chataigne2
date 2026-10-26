import { onDestroy, untrack } from "svelte";
import type { CanvasProps } from "./contracts";

/** Mutation feedback has one action generation across pointer and keyboard paths. */
export function createCanvasMutationFeedback(props: CanvasProps) {
  let message = $state("");
  let source = $derived(props.graphDocument.source);
  let owner = $derived(props.interactionScope);
  let scope = $derived({ source, owner });
  let applied: unknown;
  let generation = 0;
  let disposed = false;
  const clear = () => {
    generation++;
    message = "";
  };
  $effect.pre(() => {
    const next = scope;
    untrack(() => {
      if (next === applied) return;
      applied = next;
      clear();
    });
  });
  const dispatch = (
    requested: string,
    failed: string,
    operation: () => unknown,
    rollback?: () => void,
  ): boolean => {
    if (disposed) return false;
    const owner = scope;
    const token = ++generation;
    const reject = () => {
      if (disposed || owner !== scope) return;
      // Older failures still retire their own optimism, without replacing newer feedback.
      rollback?.();
      if (token === generation) message = failed;
    };
    message = requested;
    try {
      void Promise.resolve(operation()).catch(reject);
      return true;
    } catch {
      reject();
      return false;
    }
  };
  onDestroy(() => {
    disposed = true;
    generation++;
  });
  return {
    dispatch,
    clear,
    get message() {
      return message;
    },
  };
}
export type CanvasMutationFeedback = ReturnType<
  typeof createCanvasMutationFeedback
>;
