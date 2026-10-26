import { onDestroy, tick, untrack } from "svelte";
import type { GraphNode } from "../../types";
import type { CanvasProps } from "./contracts";
import type { CanvasGeometry } from "./geometry.svelte";
import type { CanvasKeyboard } from "./keyboard.svelte";
import type { CanvasViewport } from "./viewport.svelte";

const PAGE_SIZE = 32;

/** Logical discovery is independent of mounted canvas nodes. Only one page is retained. */
export function createCanvasNavigation(
  props: CanvasProps,
  geometry: CanvasGeometry,
  viewport: CanvasViewport,
  keyboard: CanvasKeyboard,
) {
  let open = $state(false);
  let query = $state("");
  let rows = $state.raw<readonly GraphNode[]>([]);
  let busy = $state(false);
  let nextOffset = $state<number | null>(null);
  let history = $state.raw<number[]>([0]);
  let pinnedId = $state<string | null>(null);
  let scanGeneration = 0;
  let focusGeneration = 0;
  let disposed = false;
  let focusedElement: HTMLElement | null = null;
  let source = $derived(props.graphDocument.source);
  let owner = $derived(props.interactionScope);
  let identity = $derived({ source, owner });
  let payload = $derived(props.graphDocument.revision.payload);
  let topology = $derived(props.graphDocument.revision.topology);
  let applied: unknown;

  const scan = async (offset = 0) => {
    const generation = ++scanGeneration;
    const nodes = props.graphDocument.nodes;
    const search = query.trim().toLocaleLowerCase();
    const found: GraphNode[] = [];
    busy = true;
    nextOffset = null;
    let index = offset;
    // Yield immediately for feedback, then slice sparse searches by elapsed work.
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    while (index < nodes.length) {
      if (disposed || generation !== scanGeneration) return;
      const started = performance.now();
      do {
        const node = nodes[index++];
        if (
          !search ||
          node.label.toLocaleLowerCase().includes(search) ||
          node.id.toLocaleLowerCase().includes(search)
        ) {
          if (found.length === PAGE_SIZE) {
            nextOffset = index - 1;
            rows = found;
            busy = false;
            return;
          }
          found.push(node);
        }
      } while (index < nodes.length && performance.now() - started < 4);
      if (index < nodes.length)
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
    if (disposed || generation !== scanGeneration) return;
    rows = found;
    busy = false;
  };
  const search = (value: string) => {
    if (disposed || !open) return;
    query = value;
    history = [0];
    return scan();
  };
  const show = () => {
    if (disposed) return;
    open = true;
    return search("");
  };
  const close = () => {
    open = false;
    scanGeneration++;
    rows = [];
    busy = false;
    history = [0];
    nextOffset = null;
  };
  const page = (direction: "next" | "previous") => {
    if (disposed || !open || busy) return;
    if (direction === "next") {
      if (nextOffset === null) return;
      history = [...history, nextOffset];
    } else {
      if (history.length === 1) return;
      history = history.slice(0, -1);
    }
    return scan(history.at(-1));
  };
  const reveal = async (id: string, additive = false): Promise<boolean> => {
    if (disposed) return false;
    const node = geometry.nodeById(id);
    if (!node) return false;
    close();
    const generation = ++focusGeneration;
    const scope = identity;
    pinnedId = id;
    keyboard.selectNode(id, additive);
    viewport.revealNode(node);
    const deadline = performance.now() + 2000;
    do {
      await tick();
      if (
        disposed ||
        generation !== focusGeneration ||
        scope !== identity ||
        !geometry.nodeById(id)
      )
        return false;
      const title = viewport.container?.querySelector<HTMLButtonElement>(
        `article[data-node-id="${CSS.escape(id)}"] .node-title-label`,
      );
      if (title) {
        title.focus({ preventScroll: true });
        return document.activeElement === title;
      }
      await new Promise(requestAnimationFrame);
    } while (performance.now() < deadline);
    pinnedId = null;
    return false;
  };
  const recordFocus = (target: EventTarget | null) => {
    const node =
      target instanceof HTMLElement
        ? target.closest<HTMLElement>("article[data-node-id]")
        : null;
    pinnedId = node?.dataset.nodeId ?? null;
    focusedElement = node && target instanceof HTMLElement ? target : null;
    focusGeneration++;
  };
  const leaveFocus = (event: FocusEvent) => {
    if (
      event.relatedTarget instanceof Node &&
      !viewport.container?.contains(event.relatedTarget)
    )
      recordFocus(null);
  };
  const restoreFocus = async () => {
    const element = focusedElement;
    const id = pinnedId;
    const generation = focusGeneration;
    await tick();
    if (
      disposed ||
      generation !== focusGeneration ||
      !id ||
      !element ||
      element.isConnected ||
      document.activeElement !== document.body
    )
      return;
    viewport.container
      ?.querySelector<HTMLButtonElement>(
        `article[data-node-id="${CSS.escape(id)}"] .node-title-label`,
      )
      ?.focus({ preventScroll: true });
  };
  $effect.pre(() => {
    const scope = identity;
    payload;
    topology;
    untrack(() => {
      if (scope !== applied) {
        const hadFocus = viewport.container?.contains(document.activeElement);
        close();
        pinnedId = null;
        focusedElement = null;
        focusGeneration++;
        if (hadFocus) viewport.focus();
      } else if (open) {
        history = [0];
        void scan();
      }
      applied = scope;
      if (pinnedId && !geometry.nodeById(pinnedId)) {
        pinnedId = null;
        viewport.focus();
      }
    });
  });
  onDestroy(() => {
    disposed = true;
    scanGeneration++;
    focusGeneration++;
  });
  return {
    get open() {
      return open;
    },
    get query() {
      return query;
    },
    get rows() {
      return rows;
    },
    get busy() {
      return busy;
    },
    get hasNext() {
      return nextOffset !== null;
    },
    get hasPrevious() {
      return history.length > 1;
    },
    get pageNumber() {
      return history.length;
    },
    get pinnedId() {
      return pinnedId;
    },
    show,
    close,
    search,
    page,
    reveal,
    recordFocus,
    leaveFocus,
    restoreFocus,
  };
}
export type CanvasNavigation = ReturnType<typeof createCanvasNavigation>;
