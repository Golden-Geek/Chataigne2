<script lang="ts">
  import {
    audioRoutingCurvePath,
    audioRoutingPreviewCurvePath,
    emptyAudioRoutingPatchSelection,
    findAudioRoutingPatchSnapTarget,
    isAudioRoutingActivationKey,
    selectAudioRoutingPatchEndpoint,
    type AudioRoutingPatchBinding,
    type AudioRoutingPatchConnection,
    type AudioRoutingPatchEndpoint,
    type AudioRoutingPatchSelection,
    type AudioRoutingPatchSide,
    type AudioRoutingPatchSnapTarget,
  } from "./routing";

  const SNAP_HORIZONTAL_ROWS = 1.15;
  const SNAP_HORIZONTAL_VIEWBOX_LIMIT = 32;
  const SNAP_VERTICAL_ROWS = 0.7;

  let {
    sources,
    destinations,
    connections,
    binding,
    sourceLabel = "Sources",
    destinationLabel = "Destinations",
    emptyLabel = "No routes are connected.",
  } = $props<{
    sources: readonly AudioRoutingPatchEndpoint[];
    destinations: readonly AudioRoutingPatchEndpoint[];
    connections: readonly AudioRoutingPatchConnection[];
    binding: AudioRoutingPatchBinding;
    sourceLabel?: string;
    destinationLabel?: string;
    emptyLabel?: string;
  }>();

  let selection = $state<AudioRoutingPatchSelection>(
    emptyAudioRoutingPatchSelection(),
  );
  let pendingConnection = $state<string | null>(null);
  let pendingRemoval = $state<string | null>(null);
  let pendingRename = $state<string | null>(null);
  let labelDrafts = $state<Record<string, string>>({});
  let interactionError = $state<string | null>(null);
  let pointerRoute = $state<{
    pointerId: number;
    side: AudioRoutingPatchSide;
    endpointId: string;
  } | null>(null);
  let pointerPosition = $state<{ x: number; y: number } | null>(null);
  let pointerSnap = $state<AudioRoutingPatchSnapTarget | null>(null);
  let connectionLayer = $state<SVGSVGElement | null>(null);

  let rowCount = $derived(Math.max(1, sources.length, destinations.length));
  let sourceIndexes = $derived.by(
    (): ReadonlyMap<string, number> =>
      new Map(
        sources.map((endpoint: AudioRoutingPatchEndpoint, index: number) => [
          endpoint.id,
          index,
        ]),
      ),
  );
  let destinationIndexes = $derived.by(
    (): ReadonlyMap<string, number> =>
      new Map(
        destinations.map(
          (endpoint: AudioRoutingPatchEndpoint, index: number) => [
            endpoint.id,
            index,
          ],
        ),
      ),
  );
  let sourceLabels = $derived.by(
    (): ReadonlyMap<string, string> =>
      new Map(
        sources.map((endpoint: AudioRoutingPatchEndpoint) => [
          endpoint.id,
          endpoint.label,
        ]),
      ),
  );
  let destinationLabels = $derived.by(
    (): ReadonlyMap<string, string> =>
      new Map(
        destinations.map((endpoint: AudioRoutingPatchEndpoint) => [
          endpoint.id,
          endpoint.label,
        ]),
      ),
  );
  let visibleConnections = $derived.by(
    (): readonly AudioRoutingPatchConnection[] =>
      connections.filter(
        (connection: AudioRoutingPatchConnection) =>
          sourceIndexes.has(connection.sourceId) &&
          destinationIndexes.has(connection.destinationId),
      ),
  );
  let pointerPreviewPath = $derived.by((): string | null => {
    if (!pointerRoute || !pointerPosition) return null;
    const index =
      pointerRoute.side === "source"
        ? sourceIndexes.get(pointerRoute.endpointId)
        : destinationIndexes.get(pointerRoute.endpointId);
    if (index === undefined) return null;
    return audioRoutingPreviewCurvePath(
      pointerRoute.side,
      index,
      pointerPosition.x,
      pointerPosition.y,
    );
  });

  const endpointLabel = (
    side: AudioRoutingPatchSide,
    endpointId: string,
  ): string =>
    (side === "source" ? sourceLabels : destinationLabels).get(endpointId) ??
    endpointId;

  const connectionKey = (sourceId: string, destinationId: string): string =>
    `${sourceId}\u001f${destinationId}`;

  const connectEndpoints = async (
    sourceId: string,
    destinationId: string,
  ): Promise<void> => {
    const candidate = { sourceId, destinationId };
    const key = connectionKey(candidate.sourceId, candidate.destinationId);
    pendingConnection = key;
    interactionError = null;
    const dispatched = await binding.connect(
      candidate.sourceId,
      candidate.destinationId,
    );
    if (!dispatched) interactionError = "The route request could not be sent.";
    if (pendingConnection === key) pendingConnection = null;
  };

  const activateEndpointByKeyboard = async (
    side: AudioRoutingPatchSide,
    endpointId: string,
  ): Promise<void> => {
    const result = selectAudioRoutingPatchEndpoint(selection, side, endpointId);
    selection = result.selection;
    if (result.connection) {
      await connectEndpoints(
        result.connection.sourceId,
        result.connection.destinationId,
      );
    }
  };

  const clearPointerRoute = (): void => {
    pointerRoute = null;
    pointerPosition = null;
    pointerSnap = null;
    selection = emptyAudioRoutingPatchSelection();
  };

  const updatePointerRoute = (event: PointerEvent): void => {
    const route = pointerRoute;
    if (route?.pointerId !== event.pointerId || !connectionLayer) return;
    const bounds = connectionLayer.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) {
      pointerPosition = null;
      pointerSnap = null;
      return;
    }
    const x = ((event.clientX - bounds.left) / bounds.width) * 100;
    const y = ((event.clientY - bounds.top) / bounds.height) * rowCount;
    const rowHeight = bounds.height / rowCount;
    // Scale the capture area with the rows without magnetizing the whole
    // routing canvas at narrow inspector widths.
    const horizontalSnapDistance = Math.min(
      SNAP_HORIZONTAL_VIEWBOX_LIMIT,
      (rowHeight / bounds.width) * 100 * SNAP_HORIZONTAL_ROWS,
    );
    pointerSnap = findAudioRoutingPatchSnapTarget(
      route.side,
      x,
      y,
      sources,
      destinations,
      horizontalSnapDistance,
      SNAP_VERTICAL_ROWS,
    );
    pointerPosition = pointerSnap
      ? { x: pointerSnap.x, y: pointerSnap.y }
      : {
          x: Math.max(0, Math.min(100, x)),
          y: Math.max(0, Math.min(rowCount, y)),
        };
  };

  const beginPointerRoute = (
    event: PointerEvent,
    side: AudioRoutingPatchSide,
    endpointId: string,
  ): void => {
    if (event.button !== 0 || pendingConnection !== null) return;
    event.preventDefault();
    pointerRoute = { pointerId: event.pointerId, side, endpointId };
    updatePointerRoute(event);
    selection =
      side === "source"
        ? { sourceId: endpointId, destinationId: null }
        : { sourceId: null, destinationId: endpointId };
  };

  const finishPointerRoute = (
    event: PointerEvent,
    side: AudioRoutingPatchSide,
    endpointId: string,
  ): void => {
    const origin = pointerRoute;
    if (!origin || origin.pointerId !== event.pointerId) return;
    event.preventDefault();
    event.stopPropagation();
    clearPointerRoute();
    if (origin.side === side) return;
    const sourceId = origin.side === "source" ? origin.endpointId : endpointId;
    const destinationId =
      origin.side === "destination" ? origin.endpointId : endpointId;
    void connectEndpoints(sourceId, destinationId);
  };

  const finishPointerRouteAtPointer = (event: PointerEvent): void => {
    const origin = pointerRoute;
    if (!origin || origin.pointerId !== event.pointerId) return;
    event.preventDefault();
    updatePointerRoute(event);
    const snap = pointerSnap;
    clearPointerRoute();
    if (!snap) return;
    const sourceId =
      origin.side === "source" ? origin.endpointId : snap.endpointId;
    const destinationId =
      origin.side === "destination" ? origin.endpointId : snap.endpointId;
    void connectEndpoints(sourceId, destinationId);
  };

  const removeConnection = async (
    connection: AudioRoutingPatchConnection,
  ): Promise<void> => {
    if (pendingRemoval !== null) return;
    pendingRemoval = connection.id;
    interactionError = null;
    const dispatched = await binding.disconnect(connection.id);
    if (!dispatched) interactionError = "The route request could not be sent.";
    if (pendingRemoval === connection.id) pendingRemoval = null;
  };

  const clearLabelDraft = (endpointId: string): void => {
    const nextDrafts = { ...labelDrafts };
    delete nextDrafts[endpointId];
    labelDrafts = nextDrafts;
  };

  const commitRename = async (
    side: AudioRoutingPatchSide,
    endpoint: AudioRoutingPatchEndpoint,
  ): Promise<void> => {
    const draft = labelDrafts[endpoint.id] ?? endpoint.label;
    if (draft === endpoint.label) {
      clearLabelDraft(endpoint.id);
      return;
    }
    if (pendingRename !== null) return;
    pendingRename = endpoint.id;
    interactionError = null;
    const dispatched = await binding.renameEndpoint(side, endpoint.id, draft);
    if (!dispatched) {
      interactionError = "The rename request could not be sent.";
    }
    clearLabelDraft(endpoint.id);
    if (pendingRename === endpoint.id) pendingRename = null;
  };

  const handleActivationKey = (
    event: KeyboardEvent,
    action: () => Promise<void>,
  ): void => {
    if (!isAudioRoutingActivationKey(event.key)) return;
    event.preventDefault();
    void action();
  };
</script>

<svelte:window
  onpointermove={updatePointerRoute}
  onpointerup={finishPointerRouteAtPointer}
  onpointercancel={(event) => {
    if (pointerRoute?.pointerId === event.pointerId) clearPointerRoute();
  }}
/>

<section
  class="audio-routing-patch-bay"
  aria-label="{sourceLabel} to {destinationLabel} routing"
  onpointerup={finishPointerRouteAtPointer}
  onpointercancel={clearPointerRoute}
  onpointerleave={(event) => {
    if (event.buttons === 0) clearPointerRoute();
  }}
>
  <header>
    <strong>{sourceLabel}</strong>
    <span>{visibleConnections.length} connected</span>
    <strong>{destinationLabel}</strong>
  </header>

  <div class="patch-grid" style:--audio-routing-rows={rowCount}>
    <div class="endpoint-column source-endpoints" aria-label={sourceLabel}>
      {#each sources as endpoint (endpoint.id)}
        <div class="endpoint-row source-row">
          {#if endpoint.editable}
            <input
              aria-label="Rename {endpoint.label}"
              value={labelDrafts[endpoint.id] ?? endpoint.label}
              disabled={pendingRename !== null}
              oninput={(event) => {
                labelDrafts = {
                  ...labelDrafts,
                  [endpoint.id]: event.currentTarget.value,
                };
              }}
              onchange={() => void commitRename("source", endpoint)}
            />
          {:else}
            <span title={endpoint.label}>{endpoint.label}</span>
          {/if}
          <button
            type="button"
            class:selected={selection.sourceId === endpoint.id}
            class:dragging={pointerRoute?.side === "source" &&
              pointerRoute.endpointId === endpoint.id}
            class:snap-target={pointerSnap?.side === "source" &&
              pointerSnap.endpointId === endpoint.id}
            class="endpoint"
            aria-label="Drag {endpoint.label} from {sourceLabel}"
            aria-pressed={selection.sourceId === endpoint.id}
            disabled={pendingConnection !== null}
            onpointerdown={(event) =>
              beginPointerRoute(event, "source", endpoint.id)}
            onpointerup={(event) =>
              finishPointerRoute(event, "source", endpoint.id)}
            onkeydown={(event) =>
              handleActivationKey(event, () =>
                activateEndpointByKeyboard("source", endpoint.id),
              )}
          >
            <span aria-hidden="true"></span>
          </button>
        </div>
      {/each}
    </div>

    <svg
      bind:this={connectionLayer}
      class="connection-layer"
      viewBox="0 0 100 {rowCount}"
      preserveAspectRatio="none"
      aria-label="Connected audio routes"
    >
      {#each visibleConnections as connection (connection.id)}
        {@const sourceIndex = sourceIndexes.get(connection.sourceId) ?? 0}
        {@const destinationIndex =
          destinationIndexes.get(connection.destinationId) ?? 0}
        {@const path = audioRoutingCurvePath(sourceIndex, destinationIndex)}
        <path
          class="connection-hit"
          class:pending={pendingRemoval === connection.id}
          d={path}
          role="button"
          tabindex="0"
          aria-label="Remove route from {endpointLabel(
            'source',
            connection.sourceId,
          )} to {endpointLabel('destination', connection.destinationId)}"
          onclick={() => void removeConnection(connection)}
          onkeydown={(event) =>
            handleActivationKey(event, () => removeConnection(connection))}
        ></path>
        <path class="connection-visible" d={path} aria-hidden="true"></path>
      {/each}
      {#if pointerPreviewPath}
        <path
          class="connection-preview"
          class:snapped={pointerSnap !== null}
          d={pointerPreviewPath}
          aria-hidden="true"
        ></path>
      {/if}
    </svg>

    <div
      class="endpoint-column destination-endpoints"
      aria-label={destinationLabel}
    >
      {#each destinations as endpoint (endpoint.id)}
        <div class="endpoint-row destination-row">
          <button
            type="button"
            class:selected={selection.destinationId === endpoint.id}
            class:dragging={pointerRoute?.side === "destination" &&
              pointerRoute.endpointId === endpoint.id}
            class:snap-target={pointerSnap?.side === "destination" &&
              pointerSnap.endpointId === endpoint.id}
            class="endpoint"
            aria-label="Drag {endpoint.label} from {destinationLabel}"
            aria-pressed={selection.destinationId === endpoint.id}
            disabled={pendingConnection !== null}
            onpointerdown={(event) =>
              beginPointerRoute(event, "destination", endpoint.id)}
            onpointerup={(event) =>
              finishPointerRoute(event, "destination", endpoint.id)}
            onkeydown={(event) =>
              handleActivationKey(event, () =>
                activateEndpointByKeyboard("destination", endpoint.id),
              )}
          >
            <span aria-hidden="true"></span>
          </button>
          {#if endpoint.editable}
            <input
              aria-label="Rename {endpoint.label}"
              value={labelDrafts[endpoint.id] ?? endpoint.label}
              disabled={pendingRename !== null}
              oninput={(event) => {
                labelDrafts = {
                  ...labelDrafts,
                  [endpoint.id]: event.currentTarget.value,
                };
              }}
              onchange={() => void commitRename("destination", endpoint)}
            />
          {:else}
            <span title={endpoint.label}>{endpoint.label}</span>
          {/if}
        </div>
      {/each}
    </div>
  </div>

  {#if visibleConnections.length === 0}
    <p class="empty">{emptyLabel}</p>
  {/if}
  {#if interactionError}
    <p class="interaction-error" role="alert">{interactionError}</p>
  {/if}
</section>

<style>
  .audio-routing-patch-bay {
    display: grid;
    gap: 0.55rem;
    min-inline-size: 0;
    padding: 0.65rem;
    border: 0.0625rem solid var(--audio-ui-border, #465065);
    border-radius: 0.4rem;
    background: color-mix(
      in srgb,
      var(--audio-ui-control, #202737) 55%,
      transparent
    );
    color: var(--audio-ui-text, #e8edf5);
  }

  header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 0.7rem;
    font-size: 0.75rem;
  }

  header strong:last-child {
    text-align: end;
  }

  header span,
  .empty {
    color: var(--audio-ui-muted, #aab4c4);
    font-size: 0.7rem;
  }

  .patch-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(5rem, 35%) minmax(0, 1fr);
    min-block-size: calc(var(--audio-routing-rows) * 2.25rem);
  }

  .endpoint-column {
    display: grid;
    grid-auto-rows: 2.25rem;
    align-content: start;
    min-inline-size: 0;
  }

  .endpoint-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-inline-size: 0;
  }

  .source-row {
    justify-content: end;
  }

  .destination-row {
    justify-content: start;
  }

  .endpoint-row > span {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.76rem;
  }

  input {
    inline-size: min(10rem, 100%);
    min-inline-size: 0;
    min-block-size: 1.7rem;
    padding-inline: 0.35rem;
    border: 0.0625rem solid var(--audio-ui-border, #465065);
    border-radius: 0.25rem;
    background: var(--audio-ui-control, #202737);
    color: inherit;
    font: inherit;
    font-size: 0.76rem;
  }

  .endpoint {
    display: grid;
    flex: 0 0 auto;
    place-items: center;
    inline-size: 1.25rem;
    block-size: 1.25rem;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    touch-action: none;
    user-select: none;
  }

  .endpoint span {
    inline-size: 0.58rem;
    block-size: 0.58rem;
    border: 0.08rem solid currentColor;
    border-radius: 50%;
    background: var(--audio-ui-text, #e8edf5);
    transition:
      transform 0.1s ease,
      background 0.1s ease;
  }

  .endpoint:hover span,
  .endpoint.selected span,
  .endpoint.dragging span,
  .endpoint.snap-target span {
    transform: scale(1.2);
    background: var(--audio-ui-focus, #70b7ff);
  }

  .endpoint.snap-target span {
    transform: scale(1.4);
    box-shadow: 0 0 0 0.22rem
      color-mix(in srgb, var(--audio-ui-focus, #70b7ff) 30%, transparent);
  }

  .endpoint:disabled {
    cursor: default;
    opacity: 0.55;
  }

  .connection-layer {
    display: block;
    inline-size: 100%;
    block-size: calc(var(--audio-routing-rows) * 2.25rem);
    overflow: visible;
  }

  .connection-visible,
  .connection-hit,
  .connection-preview {
    fill: none;
    vector-effect: non-scaling-stroke;
  }

  .connection-visible {
    stroke: var(--audio-ui-route, #d5d8dc);
    stroke-width: 0.08rem;
    pointer-events: none;
  }

  .connection-hit {
    stroke: transparent;
    stroke-width: 0.85rem;
    cursor: pointer;
    pointer-events: stroke;
  }

  .connection-hit:hover + .connection-visible,
  .connection-hit:focus-visible + .connection-visible {
    stroke: var(--audio-ui-focus, #70b7ff);
  }

  .connection-hit.pending {
    pointer-events: none;
  }

  .connection-preview {
    stroke: var(--audio-ui-focus, #70b7ff);
    stroke-width: 0.13rem;
    stroke-dasharray: 0.35rem 0.22rem;
    pointer-events: none;
  }

  .connection-preview.snapped {
    stroke-width: 0.16rem;
    stroke-dasharray: none;
  }

  .endpoint:focus-visible,
  input:focus-visible {
    outline: 0.15rem solid var(--audio-ui-focus, #70b7ff);
    outline-offset: 0.1rem;
  }

  .empty,
  .interaction-error {
    margin: 0;
    padding: 0.45rem 0.55rem;
    border-radius: 0.3rem;
  }

  .interaction-error {
    background: #3c2328;
    color: #ffd8d8;
    font-size: 0.74rem;
  }

  @media (max-width: 34rem) {
    .patch-grid {
      grid-template-columns: minmax(0, 1fr) minmax(3rem, 24%) minmax(0, 1fr);
    }
  }
</style>
