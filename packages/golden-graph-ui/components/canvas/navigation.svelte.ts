import { onDestroy, tick, untrack } from "svelte";
import type { GraphNavigationItem } from "../../types";
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
  let rows = $state.raw<readonly GraphNavigationItem[]>([]);
  let busy = $state(false);
  let nextOffset = $state<number | null>(null);
  let history = $state.raw<number[]>([0]);
  let pinnedId = $state<string | null>(null);
  let scanGeneration = 0;
  let focusGeneration = 0;
  let disposed = false;
  let focusedElement: HTMLElement | SVGElement | null = null;
  let source = $derived(
    props.worldNavigation
      ? props.worldNavigation.source
      : props.graphDocument.source,
  );
  let items = $derived(
    props.worldNavigation?.items ?? props.graphDocument.nodes,
  );
  let customIndex = $derived(
    new Map(props.worldNavigation?.items.map((item) => [item.id, item]) ?? []),
  );
  let owner = $derived(props.interactionScope);
  let identity = $derived({ source, owner, world: props.worldNavigation });
  let payload = $derived(props.graphDocument.revision.payload);
  let topology = $derived(props.graphDocument.revision.topology);
  let applied: unknown;

  const hasItem = (id: string) =>
    props.worldNavigation ? customIndex.has(id) : !!geometry.nodeById(id);
  const focusTarget = (id: string) => {
    const canvas = viewport.container;
    if (!canvas) return null;
    const target = props.worldNavigation
      ? props.worldNavigation.focusTarget(id, canvas)
      : canvas.querySelector<HTMLButtonElement>(
          `article[data-node-id="${CSS.escape(id)}"] .node-title-label`,
        );
    return target && canvas.contains(target) ? target : null;
  };

  const scan = async (offset = 0) => {
    const generation = ++scanGeneration;
    const nodes = items;
    const search = query.trim().toLocaleLowerCase();
    const found: GraphNavigationItem[] = [];
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
    const custom = props.worldNavigation;
    const nativeNode = custom ? null : geometry.nodeById(id);
    const node = custom ? customIndex.get(id) : nativeNode;
    if (!node) return false;
    const bounds = custom?.bounds(id);
    if (custom && !bounds) return false;
    close();
    const generation = ++focusGeneration;
    const scope = identity;
    pinnedId = id;
    if (custom && bounds) {
      custom.select(id, additive);
      viewport.revealBounds(bounds);
    } else {
      keyboard.selectNode(id, additive);
      if (nativeNode) viewport.revealNode(nativeNode);
    }
    const deadline = performance.now() + 2000;
    do {
      await tick();
      if (
        disposed ||
        generation !== focusGeneration ||
        scope !== identity ||
        !hasItem(id)
      )
        return false;
      const title = focusTarget(id);
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
    const element =
      target instanceof HTMLElement || target instanceof SVGElement
        ? target
        : null;
    const id = element
      ? props.worldNavigation
        ? props.worldNavigation.itemIdForElement(element)
        : (element.closest<HTMLElement>("article[data-node-id]")?.dataset
            .nodeId ?? null)
      : null;
    pinnedId = id && hasItem(id) ? id : null;
    focusedElement = pinnedId ? element : null;
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
    focusTarget(id)?.focus({ preventScroll: true });
  };
  $effect.pre(() => {
    const scope = identity;
    if (props.worldNavigation) items;
    else {
      payload;
      topology;
    }
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
      if (pinnedId && !hasItem(pinnedId)) {
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
    get itemCount() {
      return items.length;
    },
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
