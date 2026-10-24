import {
  DEFAULT_MIN_ZOOM,
  DEFAULT_MAX_ZOOM,
  CHECKER_CELL_REM,
  FRAME_PADDING_REM,
  CAMERA_ANIMATION_MS,
  SCROLL_TARGET_ACTIVATION_MS,
  finiteNumber,
  clamp,
  type CanvasProps,
} from "./contracts";
import type {
  GraphCamera,
  GraphNode,
  GraphNodePosition,
  GraphWorldBounds,
} from "../../types";
import { onMount } from "svelte";
import {
  cameraAtZoom as cameraAtZoomForAnchor,
  cameraForBounds,
  graphBoundsForNodes,
  visibleViewport as calculateVisibleViewport,
} from "../../camera-geometry";

interface ViewportScene {
  readonly effectiveNodes: GraphNode[];
  readonly selectedIds: ReadonlySet<string>;
  nodeWidth(node: GraphNode): number;
  nodeHeight(node: GraphNode): number;
}
export function createCanvasViewport(props: CanvasProps, scene: ViewportScene) {
  let effectiveMaxZoom = $derived(
    Math.max(0.01, finiteNumber(props.maxZoom, DEFAULT_MAX_ZOOM)),
  );

  let effectiveMinZoom = $derived(
    Math.min(
      effectiveMaxZoom,
      Math.max(0.01, finiteNumber(props.minZoom, DEFAULT_MIN_ZOOM)),
    ),
  );

  const normalizeCamera = (value: GraphCamera | undefined): GraphCamera => ({
    x: finiteNumber(value?.x, 0),
    y: finiteNumber(value?.y, 0),
    zoom: Math.min(
      effectiveMaxZoom,
      Math.max(effectiveMinZoom, finiteNumber(value?.zoom, 1)),
    ),
  });

  const cameraKey = (value: GraphCamera): string =>
    `${value.x}:${value.y}:${value.zoom}`;

  const snapScreenPx = (value: number): number => {
    const scale =
      typeof window === "undefined"
        ? 1
        : Math.max(1, window.devicePixelRatio || 1);
    return Math.round(value * scale) / scale;
  };

  let container: HTMLDivElement | null = $state(null);

  let camera = $state<GraphCamera>({ x: 0, y: 0, zoom: 1 });

  let appliedInitialCameraKey = $state<string | null>(null);

  let remPx = $state(16);

  let viewportWidth = $state(1);

  let viewportHeight = $state(1);

  let hoveredScrollTarget: HTMLElement | null = null;

  let wheelScrollTarget: HTMLElement | null = null;

  let scrollActivationTimer: ReturnType<typeof setTimeout> | null = null;

  let animationFrame: number | null = null;

  let checkerCellSizePx = $derived(
    Math.max(8, remPx * CHECKER_CELL_REM * camera.zoom),
  );

  let checkerSizePx = $derived(checkerCellSizePx * 2);

  let snappedCameraX = $derived(snapScreenPx(camera.x));

  let snappedCameraY = $derived(snapScreenPx(camera.y));

  let panStyle = $derived(
    `translate(${snappedCameraX}px, ${snappedCameraY}px)`,
  );

  const visibleViewport = () =>
    calculateVisibleViewport(
      props.viewportInset,
      viewportWidth,
      viewportHeight,
    );

  const visibleViewportCenter = (): GraphNodePosition => {
    const viewport = visibleViewport();
    return {
      x: viewport.x + viewport.width * 0.5,
      y: viewport.y + viewport.height * 0.5,
    };
  };

  const camerasMatch = (left: GraphCamera, right: GraphCamera): boolean =>
    Math.abs(left.x - right.x) < 0.0001 &&
    Math.abs(left.y - right.y) < 0.0001 &&
    Math.abs(left.zoom - right.zoom) < 0.0001;

  const applyCamera = (next: GraphCamera, notify = true): void => {
    const normalized = normalizeCamera(next);
    camera = normalized;
    if (notify) {
      props.onCameraChange?.({ ...normalized });
    }
  };

  const clientToWorldPx = (
    clientX: number,
    clientY: number,
  ): { x: number; y: number } => {
    const bounds = container?.getBoundingClientRect();
    return {
      x: (clientX - (bounds?.left ?? 0) - camera.x) / camera.zoom,
      y: (clientY - (bounds?.top ?? 0) - camera.y) / camera.zoom,
    };
  };

  const cameraAtZoom = (
    zoom: number,
    anchorX: number,
    anchorY: number,
  ): GraphCamera => {
    return cameraAtZoomForAnchor(
      camera,
      zoom,
      anchorX,
      anchorY,
      effectiveMinZoom,
      effectiveMaxZoom,
    );
  };

  const cancelCameraAnimation = (): void => {
    if (animationFrame !== null) {
      cancelAnimationFrame(animationFrame);
      animationFrame = null;
    }
  };

  $effect(() => {
    if (!props.initialCamera) {
      return;
    }
    const normalized = normalizeCamera(props.initialCamera);
    const key = cameraKey(normalized);
    if (key === appliedInitialCameraKey) {
      return;
    }
    appliedInitialCameraKey = key;
    if (!camerasMatch(camera, normalized)) {
      cancelCameraAnimation();
      applyCamera(normalized, false);
    }
  });

  const animateCamera = (target: GraphCamera): void => {
    cancelCameraAnimation();
    const source = { ...camera };
    const startedAt = performance.now();
    const tick = (now: number): void => {
      const progress = clamp((now - startedAt) / CAMERA_ANIMATION_MS, 0, 1);
      const eased = 1 - (1 - progress) ** 3;
      applyCamera({
        x: source.x + (target.x - source.x) * eased,
        y: source.y + (target.y - source.y) * eased,
        zoom: source.zoom + (target.zoom - source.zoom) * eased,
      });
      if (progress < 1) {
        animationFrame = requestAnimationFrame(tick);
      } else {
        animationFrame = null;
      }
    };
    animationFrame = requestAnimationFrame(tick);
  };

  const setZoom = (zoom: number): void => {
    const center = visibleViewportCenter();
    cancelCameraAnimation();
    applyCamera(cameraAtZoom(zoom, center.x, center.y));
  };

  const resetZoom = (): void => {
    const center = visibleViewportCenter();
    animateCamera(cameraAtZoom(1, center.x, center.y));
  };

  const handleZoomInput = (event: Event): void => {
    setZoom((event.currentTarget as HTMLInputElement).valueAsNumber / 100);
  };

  const frameBounds = (bounds: GraphWorldBounds | null): boolean => {
    if (!container || bounds === null) {
      return false;
    }
    const target = cameraForBounds(
      bounds,
      visibleViewport(),
      remPx,
      FRAME_PADDING_REM,
      effectiveMinZoom,
      effectiveMaxZoom,
    );
    if (!target) {
      return false;
    }
    animateCamera(target);
    return true;
  };

  const frameNodes = (candidates: GraphNode[]): boolean =>
    frameBounds(
      graphBoundsForNodes(candidates, scene.nodeWidth, scene.nodeHeight),
    );

  const frameSelection = (): boolean => {
    const selected = scene.effectiveNodes.filter((node) =>
      scene.selectedIds.has(node.id),
    );
    if (selected.length > 0 || scene.effectiveNodes.length > 0) {
      return frameNodes(selected.length > 0 ? selected : scene.effectiveNodes);
    }
    return frameBounds(props.worldBounds ?? null);
  };

  const home = (): boolean =>
    scene.effectiveNodes.length > 0
      ? frameNodes(scene.effectiveNodes)
      : frameBounds(props.worldBounds ?? null);

  const focus = (): void => {
    container?.focus();
  };

  const clientToWorld = (
    clientX: number,
    clientY: number,
  ): GraphNodePosition => {
    const world = clientToWorldPx(clientX, clientY);
    return {
      x: world.x / remPx,
      y: world.y / remPx,
    };
  };

  const viewportCenter = (): GraphNodePosition => {
    const center = visibleViewportCenter();
    return {
      x: (center.x - camera.x) / camera.zoom / remPx,
      y: (center.y - camera.y) / camera.zoom / remPx,
    };
  };

  const isScrollableElement = (element: HTMLElement): boolean => {
    if (element.scrollHeight <= element.clientHeight) {
      return false;
    }
    const overflowY = getComputedStyle(element).overflowY;
    return overflowY === "auto" || overflowY === "scroll";
  };

  const findScrollableUnderPointer = (
    target: EventTarget | null,
  ): HTMLElement | null => {
    let element = target instanceof HTMLElement ? target : null;
    while (element && element !== container) {
      if (isScrollableElement(element)) {
        return element;
      }
      element = element.parentElement;
    }
    return null;
  };

  const clearScrollActivationTimer = (): void => {
    if (scrollActivationTimer !== null) {
      clearTimeout(scrollActivationTimer);
      scrollActivationTimer = null;
    }
  };

  // Briefly passing the pointer over a scrollable area keeps wheel = zoom; only once the
  // pointer dwells there past SCROLL_TARGET_ACTIVATION_MS does wheel switch to scrolling it.
  const updateScrollHoverTarget = (event: PointerEvent): void => {
    const candidate = findScrollableUnderPointer(event.target);
    if (candidate === hoveredScrollTarget) {
      return;
    }
    hoveredScrollTarget = candidate;
    wheelScrollTarget = null;
    clearScrollActivationTimer();
    if (candidate) {
      scrollActivationTimer = setTimeout(() => {
        wheelScrollTarget = candidate;
        scrollActivationTimer = null;
      }, SCROLL_TARGET_ACTIVATION_MS);
    }
  };

  const resetScrollHoverTarget = (): void => {
    hoveredScrollTarget = null;
    wheelScrollTarget = null;
    clearScrollActivationTimer();
  };

  const handleWheel = (event: WheelEvent): void => {
    if (
      wheelScrollTarget &&
      event.target instanceof Node &&
      wheelScrollTarget.contains(event.target)
    ) {
      return;
    }
    event.preventDefault();
    const bounds = container?.getBoundingClientRect();
    if (!bounds) {
      return;
    }
    const pointerX = event.clientX - bounds.left;
    const pointerY = event.clientY - bounds.top;
    cancelCameraAnimation();
    applyCamera(
      cameraAtZoom(
        camera.zoom * Math.exp(-event.deltaY * 0.0014),
        pointerX,
        pointerY,
      ),
    );
  };

  onMount(() => {
    if (!container) {
      return;
    }
    remPx =
      Number.parseFloat(getComputedStyle(document.documentElement).fontSize) ||
      16;
    const observer = new ResizeObserver(([entry]) => {
      viewportWidth = Math.max(1, entry.contentRect.width);
      viewportHeight = Math.max(1, entry.contentRect.height);
    });
    observer.observe(container);
    if (props.autoHomeOnMount && !props.initialCamera) {
      requestAnimationFrame(() => home());
    }
    return () => {
      observer.disconnect();
      cancelCameraAnimation();
      clearScrollActivationTimer();
    };
  });

  return {
    get effectiveMaxZoom() {
      return effectiveMaxZoom;
    },
    get effectiveMinZoom() {
      return effectiveMinZoom;
    },
    get container() {
      return container;
    },
    set container(value: typeof container) {
      container = value;
    },
    get camera() {
      return camera;
    },
    get remPx() {
      return remPx;
    },
    get viewportWidth() {
      return viewportWidth;
    },
    get viewportHeight() {
      return viewportHeight;
    },
    get checkerSizePx() {
      return checkerSizePx;
    },
    get snappedCameraX() {
      return snappedCameraX;
    },
    get snappedCameraY() {
      return snappedCameraY;
    },
    get panStyle() {
      return panStyle;
    },
    applyCamera,
    clientToWorldPx,
    resetZoom,
    handleZoomInput,
    frameSelection,
    home,
    clientToWorld,
    viewportCenter,
    updateScrollHoverTarget,
    resetScrollHoverTarget,
    handleWheel,
    focus,
  };
}

export type CanvasViewport = ReturnType<typeof createCanvasViewport>;
