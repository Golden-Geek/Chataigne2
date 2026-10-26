<script lang="ts">
	import type {
		SpatializerCanvas,
		SpatialEndpoint,
		Point2,
		ValueLayout,
		RelatedValue
	} from './spatializer/types';
	import {
		createSpatializerProjection,
		enumValue,
		enumLabel,
		valueLayoutFromEnum,
		relatedValueAmount
	} from './spatializer/projection.svelte';
	import {
		createSpatializerGeometry,
		graphPositionToSpatialPosition
	} from './spatializer/geometry.svelte';
	import { createSpatializerInteraction } from './spatializer/interaction.svelte';
	import SpatializerWorld from './spatializer/SpatializerWorld.svelte';
	import {
		SPATIALIZER_MODULE_TYPE,
		SOURCE_LIST_TYPE,
		TARGET_LIST_TYPE,
		SOURCE_TYPE,
		TARGET_TYPE,
		VALUE_LAYOUT_DECL_ID,
		VALUE_LAYOUT_TARGET_CENTRIC,
		MODE_VORONOI,
		CAMERA_PERSIST_DELAY_MS,
		SPATIALIZER_UNIT_REM,
		SPATIALIZER_MIN_ZOOM,
		MIN_INSPECTOR_WIDTH,
		MAX_INSPECTOR_WIDTH,
		DEFAULT_INSPECTOR_WIDTH
	} from './spatializer/constants';

	import { onMount } from 'svelte';
	import Slider from 'golden_ui/components/common/Slider.svelte';
	import {
		GraphCanvas,
		GraphPresentationAdapter,
		type GraphCamera,
		type GraphNodePosition,
		type GraphWorldContentContext
	} from 'golden_graph_ui';
	import type { NodeId, PanelProps, PanelState, UiNodeDto } from 'golden_ui';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from 'golden_ui/dockview/panel-persistence';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { appState } from 'golden_ui/store/workbench.svelte';

	type EditorParams = {
		moduleNodeId?: NodeId;
	};

	interface SpatializerEditorPersistedState {
		camera?: GraphCamera;
		debugView?: boolean;
		spatialUnitRem?: number;
	}

	let props: PanelProps = $props();
	let updatedPanelState = $state<PanelState | null>(null);
	let panelState = $derived(
		updatedPanelState ?? {
			panelId: props.panelId,
			panelType: props.panelType,
			title: props.title,
			params: props.params
		}
	);
	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};

	const persistedDebugView = (params: PanelState['params']): boolean =>
		readPanelPersistedState<SpatializerEditorPersistedState>(params).debugView === true;

	let panelRoot: HTMLElement | null = $state(null);
	let worldSvg: SVGSVGElement | null = $state(null);
	let graphCanvas = $state<SpatializerCanvas | null>(null);
	const interaction = createSpatializerInteraction(() => ({
		session,
		module: activeModule,
		space: prefer3d ? 'xz' : 'xy',
		rawEndpoints,
		graphCanvas,
		worldSvg,
		selectEndpoint
	}));
	const graphDocument = new GraphPresentationAdapter().update([], []);
	let debugView = $state(false);
	let appliedPersistedDebugView = $state(false);
	let inspectorVisible = $state(true);
	let inspectorWidth = $state(DEFAULT_INSPECTOR_WIDTH);
	let pendingCamera: GraphCamera | null = null;
	let cameraPersistenceTimer: ReturnType<typeof setTimeout> | null = null;

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let panelParams = $derived((panelState.params ?? {}) as EditorParams);

	const finiteNumber = (value: unknown): value is number =>
		typeof value === 'number' && Number.isFinite(value);

	const graphCamera = (value: unknown): GraphCamera | undefined => {
		if (typeof value !== 'object' || value === null) {
			return undefined;
		}
		const candidate = value as Record<string, unknown>;
		if (!finiteNumber(candidate.x) || !finiteNumber(candidate.y) || !finiteNumber(candidate.zoom)) {
			return undefined;
		}
		return { x: candidate.x, y: candidate.y, zoom: candidate.zoom };
	};

	const camerasMatch = (left: GraphCamera | undefined, right: GraphCamera): boolean =>
		left !== undefined &&
		Math.abs(left.x - right.x) < 0.0001 &&
		Math.abs(left.y - right.y) < 0.0001 &&
		Math.abs(left.zoom - right.zoom) < 0.0001;

	const persistedCameraForScale = (params: PanelState['params']): GraphCamera | undefined => {
		const persisted = readPanelPersistedState<SpatializerEditorPersistedState>(params);
		return persisted.spatialUnitRem === SPATIALIZER_UNIT_REM
			? graphCamera(persisted.camera)
			: undefined;
	};

	let initialCamera = $derived.by(() => persistedCameraForScale(panelState.params));

	$effect(() => {
		const persisted = persistedDebugView(panelState.params);
		if (persisted !== appliedPersistedDebugView) {
			appliedPersistedDebugView = persisted;
			debugView = persisted;
		}
	});

	const clampInspectorWidth = (value: number): number =>
		Math.max(MIN_INSPECTOR_WIDTH, Math.min(MAX_INSPECTOR_WIDTH, value));

	const { childByKey, childByType, childNodes, endpointFromNode } = createSpatializerProjection(
		() => graph
	);
	let modules = $derived.by((): UiNodeDto[] => {
		if (!graph) {
			return [];
		}
		return [...graph.nodesById.values()]
			.filter((candidate) => candidate.node_type === SPATIALIZER_MODULE_TYPE)
			.sort((left, right) => left.meta.label.localeCompare(right.meta.label));
	});

	let activeModule = $derived.by((): UiNodeDto | null => {
		if (modules.length === 0) {
			return null;
		}
		return modules.find((module) => module.node_id === panelParams.moduleNodeId) ?? modules[0];
	});

	let parametersFolder = $derived(childByKey(activeModule, 'parameters'));
	let valuesFolder = $derived(childByKey(activeModule, 'values'));
	let dimensionsParam = $derived(childByKey(parametersFolder, 'dimensions'));
	let modeParam = $derived(childByKey(parametersFolder, 'mode'));
	let valueLayoutParam = $derived(childByKey(parametersFolder, VALUE_LAYOUT_DECL_ID));
	let sourceList = $derived(childByType(parametersFolder, SOURCE_LIST_TYPE));
	let targetList = $derived(childByType(parametersFolder, TARGET_LIST_TYPE));
	let dimensionMode = $derived(enumValue(dimensionsParam) || '2d');
	let spatializerMode = $derived(enumValue(modeParam) || MODE_VORONOI);
	let valueLayout = $derived.by((): ValueLayout =>
		valueLayoutFromEnum(enumValue(valueLayoutParam))
	);
	let prefer3d = $derived(dimensionMode === '3d');

	let rawEndpoints = $derived.by((): SpatialEndpoint[] => {
		const sources = childNodes(sourceList)
			.filter((node) => node.node_type === SOURCE_TYPE)
			.map((node, index) => endpointFromNode(node, 'source', index, prefer3d));
		const targets = childNodes(targetList)
			.filter((node) => node.node_type === TARGET_TYPE)
			.map((node, index) => endpointFromNode(node, 'target', index, prefer3d));
		return [...targets, ...sources];
	});

	let endpoints = $derived(
		rawEndpoints.map((endpoint) => ({
			...endpoint,
			...(interaction.positionPreviews[endpoint.key] ?? {}),
			radius: interaction.radiusPreviews[endpoint.key] ?? endpoint.radius,
			freezeRadius: interaction.freezeRadiusPreviews[endpoint.key] ?? endpoint.freezeRadius
		}))
	);

	let targets = $derived(endpoints.filter((endpoint) => endpoint.kind === 'target'));
	let sources = $derived(endpoints.filter((endpoint) => endpoint.kind === 'source'));
	let enabledTargets = $derived(targets.filter((endpoint) => endpoint.enabled));

	let selectedEndpoint = $derived.by((): SpatialEndpoint | null => {
		const selectedNodeId = session?.selectedNodeId;
		if (selectedNodeId === null || selectedNodeId === undefined) {
			return null;
		}
		return endpoints.find((endpoint) => endpoint.node.node_id === selectedNodeId) ?? null;
	});

	let dimensionLabel = $derived(prefer3d ? 'XZ' : 'XY');
	let modeLabel = $derived(enumLabel(modeParam) || 'Voronoi');

	const valueParameterInFolder = (folderDeclId: string, valueDeclId: string): UiNodeDto | null => {
		const folder = childByKey(valuesFolder, folderDeclId);
		if (!folder) {
			return null;
		}
		const valueNode = childByKey(folder, valueDeclId);
		return valueNode?.data.kind === 'parameter' ? valueNode : null;
	};

	const valueParameterForPair = (
		target: SpatialEndpoint,
		source: SpatialEndpoint
	): UiNodeDto | null => {
		const targetCentricValue = (): UiNodeDto | null =>
			valueParameterInFolder(target.valueDeclId, source.valueDeclId);
		const sourceCentricValue = (): UiNodeDto | null =>
			valueParameterInFolder(source.valueDeclId, target.valueDeclId);
		return valueLayout === VALUE_LAYOUT_TARGET_CENTRIC
			? (targetCentricValue() ?? sourceCentricValue())
			: (sourceCentricValue() ?? targetCentricValue());
	};

	let relatedValues = $derived.by((): RelatedValue[] => {
		if (!selectedEndpoint) {
			return [];
		}
		if (selectedEndpoint.kind === 'source') {
			return targets.map((target) => ({
				key: `${selectedEndpoint.key}:${target.key}`,
				endpoint: target,
				valueParam: valueParameterForPair(target, selectedEndpoint)
			}));
		}
		return sources.map((source) => ({
			key: `${selectedEndpoint.key}:${source.key}`,
			endpoint: source,
			valueParam: valueParameterForPair(selectedEndpoint, source)
		}));
	});

	const geometry = createSpatializerGeometry(() => ({
		endpoints,
		targets,
		sources,
		enabledTargets,
		spatializerMode,
		debugView,
		selectedEndpoint,
		relatedValues,
		valueParameterForPair
	}));
	$effect(() => {
		const nextTitle = activeModule
			? `Spatializer: ${activeModule.meta.label}`
			: 'Spatializer Editor';
		if (panelState.title !== nextTitle) {
			props.panelApi.setTitle(nextTitle);
		}
	});

	const panelOwnsFocus = (): boolean =>
		panelRoot !== null &&
		document.activeElement !== null &&
		panelRoot.contains(document.activeElement);

	const flushCameraPersistence = (): void => {
		if (cameraPersistenceTimer !== null) {
			clearTimeout(cameraPersistenceTimer);
			cameraPersistenceTimer = null;
		}
		const nextCamera = pendingCamera;
		pendingCamera = null;
		if (!nextCamera) {
			return;
		}
		const currentCamera = persistedCameraForScale(props.panelApi.getParams());
		if (camerasMatch(currentCamera, nextCamera)) {
			return;
		}
		writePanelPersistedState(props.panelApi, {
			camera: nextCamera,
			spatialUnitRem: SPATIALIZER_UNIT_REM
		});
	};

	const persistCamera = (camera: GraphCamera): void => {
		pendingCamera = { ...camera };
		if (cameraPersistenceTimer !== null) {
			clearTimeout(cameraPersistenceTimer);
		}
		cameraPersistenceTimer = setTimeout(flushCameraPersistence, CAMERA_PERSIST_DELAY_MS);
	};

	const setDebugView = (event: Event): void => {
		const enabled = (event.currentTarget as HTMLInputElement).checked;
		debugView = enabled;
		appliedPersistedDebugView = enabled;
		writePanelPersistedState(props.panelApi, { debugView: enabled });
	};

	const selectModule = (event: Event): void => {
		const value = Number((event.currentTarget as HTMLSelectElement).value);
		if (!Number.isSafeInteger(value)) {
			return;
		}
		props.panelApi.updateParams({ ...panelParams, moduleNodeId: value });
		session?.selectNode(value, 'REPLACE');
	};

	const selectEndpoint = (endpoint: SpatialEndpoint): void => {
		session?.selectNode(endpoint.node.node_id, 'REPLACE');
	};

	const startInspectorResize = (event: PointerEvent): void => {
		event.preventDefault();
		event.stopPropagation();
		const startX = event.clientX;
		const startWidth = inspectorWidth;
		const handle = event.currentTarget as HTMLElement;
		handle.setPointerCapture(event.pointerId);
		handle.onpointermove = (moveEvent) => {
			inspectorWidth = clampInspectorWidth(startWidth + moveEvent.clientX - startX);
		};
		handle.onpointerup = handle.onpointercancel = (endEvent) => {
			if (handle.hasPointerCapture(endEvent.pointerId)) {
				handle.releasePointerCapture(endEvent.pointerId);
			}
			handle.onpointermove = null;
			handle.onpointerup = null;
			handle.onpointercancel = null;
		};
	};

	export const createTargetAtWorld = (position: GraphNodePosition) =>
		interaction.createTarget(graphPositionToSpatialPosition(position));
	const createTargetAt = async (event: MouseEvent, position: GraphNodePosition): Promise<void> => {
		event.preventDefault();
		event.stopPropagation();
		graphCanvas?.focus();
		try {
			await createTargetAtWorld(position);
		} catch {
			/* The interaction owner publishes failure feedback. */
		}
	};
	export const createTargetInView = () => {
		const rect = panelRoot?.querySelector('.canvas-region')?.getBoundingClientRect();
		if (!rect || !graphCanvas) return Promise.reject(new Error('Spatializer view is unavailable.'));
		const x = rect.left + (rect.width + (inspectorVisible ? inspectorWidth : 0)) / 2;
		return createTargetAtWorld(graphCanvas.clientToWorld(x, rect.top + rect.height / 2));
	};
	export const editEndpointKey = (
		id: number,
		key: string,
		shift = false,
		kind: 'position' | 'radius' | 'freezeRadius' = 'position'
	) => {
		const endpoint = rawEndpoints.find((endpoint) => endpoint.node.node_id === id);
		return endpoint ? interaction.keyboard(endpoint, key, shift, kind) : Promise.resolve(false);
	};
	export const beginEndpointGesture = (
		id: number,
		kind: 'position' | 'radius' | 'freezeRadius',
		point: Point2
	) => {
		const endpoint = endpoints.find((endpoint) => endpoint.node.node_id === id);
		return endpoint ? interaction.begin(endpoint, kind, point) : false;
	};
	export const updateEndpointGesture = (point: Point2) => interaction.update(point);
	export const moveEndpoints = (moves: { id: number; position: Point2 }[]) =>
		interaction.move(
			moves.map((move) => {
				const endpoint = session?.graph.state.nodesById.get(move.id);
				if (!endpoint) throw new Error('Spatializer endpoint is unavailable.');
				return { endpoint, position: { ...move.position } };
			})
		);
	export const finishEndpointGesture = (cancelled = false) => interaction.finish(cancelled);

	const isSelected = (endpoint: SpatialEndpoint): boolean =>
		session?.selectedNodeId === endpoint.node.node_id;

	const endpointKindLabel = (endpoint: SpatialEndpoint): string =>
		endpoint.kind === 'source' ? 'Source' : 'Target';

	const relatedValuesLabel = (endpoint: SpatialEndpoint): string =>
		endpoint.kind === 'source' ? 'Targets' : 'Sources';

	onMount(() => {
		const unregisterFrame = registerCommandHandler(
			'view.frame',
			() => (panelOwnsFocus() ? (graphCanvas?.frameSelection() ?? false) : false),
			{ priority: 100 }
		);
		const unregisterHome = registerCommandHandler(
			'view.home',
			() => (panelOwnsFocus() ? (graphCanvas?.home() ?? false) : false),
			{ priority: 100 }
		);
		return () => {
			interaction.dispose();
			unregisterFrame();
			unregisterHome();
			flushCameraPersistence();
		};
	});
</script>

{#snippet spatializerWorld(world: GraphWorldContentContext)}
	<SpatializerWorld
		{world}
		{geometry}
		{interaction}
		{targets}
		{sources}
		{spatializerMode}
		{debugView}
		{selectedEndpoint}
		{isSelected}
		bind:svg={worldSvg} />
{/snippet}

<section bind:this={panelRoot} class="spatializer-editor-panel" aria-label={panelState.title}>
	<header class="editor-toolbar">
		<label class="module-picker">
			<span>Module</span>
			<select value={String(activeModule?.node_id ?? '')} onchange={selectModule}>
				{#each modules as module (module.node_id)}
					<option value={module.node_id}>{module.meta.label}</option>
				{/each}
			</select>
		</label>
		<button
			type="button"
			class="toolbar-chip"
			disabled={!activeModule}
			onclick={() => void createTargetInView().catch(() => undefined)}>Add target</button>
		<span class="toolbar-chip">{dimensionLabel}</span>
		<span class="toolbar-chip">{modeLabel}</span>
		<label class="debug-toggle">
			<input type="checkbox" checked={debugView} onchange={setDebugView} />
			<span>Debug</span>
		</label>
	</header>

	<p role="status" aria-live="polite" class="edit-feedback">{interaction.message}</p>
	{#if activeModule}
		<div class="editor-body">
			<div class="canvas-region">
				<GraphCanvas
					bind:this={graphCanvas}
					{graphDocument}
					interactionHelp="Use Enter or Space to select an endpoint. Arrow keys move it; Shift moves farther. Arrow keys on a radius handle resize it. Use Add target to create at the view center."
					worldContent={spatializerWorld}
					worldBounds={geometry.graphWorldBounds}
					{initialCamera}
					minZoom={SPATIALIZER_MIN_ZOOM}
					onCameraChange={persistCamera}
					onBackgroundDoubleClick={createTargetAt}
					viewportInset={{ left: inspectorVisible ? inspectorWidth : 0 }}
					autoHomeOnMount={true}
					emptyLabel="" />
			</div>

			<aside
				class="endpoint-inspector"
				class:visible={inspectorVisible}
				style:width={`${inspectorWidth}px`}
				aria-label="Spatializer preview">
				<div class="endpoint-inspector-heading">
					<button
						type="button"
						class="endpoint-inspector-toggle"
						aria-pressed={inspectorVisible}
						title="Hide preview panel"
						onclick={() => (inspectorVisible = false)}>
						<span class="endpoint-inspector-chevron">‹</span>
						<span class="endpoint-inspector-label">Preview</span>
					</button>
				</div>
				<div class="endpoint-inspector-body">
					{#if selectedEndpoint}
						<div class="endpoint-summary">
							<span class="endpoint-swatch" style:background={selectedEndpoint.color}></span>
							<div>
								<strong>{selectedEndpoint.node.meta.label}</strong>
								<span>{endpointKindLabel(selectedEndpoint)}</span>
							</div>
						</div>

						<section class="related-values" aria-label="Computed values">
							<header>
								<span>{relatedValuesLabel(selectedEndpoint)}</span>
								<span>{relatedValues.length}</span>
							</header>
							<div class="related-value-list">
								{#each relatedValues as item (item.key)}
									<div class="related-value-row" class:disabled={!item.endpoint.enabled}>
										<div class="related-value-label">
											<span class="related-value-swatch" style:background={item.endpoint.color}
											></span>
											<span>{item.endpoint.node.meta.label}</span>
										</div>
										<div class="related-value-control">
											{#if item.valueParam}
												<div class="related-value-slider">
													<Slider
														value={relatedValueAmount(item.valueParam)}
														min={0}
														max={1}
														step={0.001}
														readOnly={true}
														disabled={!item.endpoint.enabled}
														fgColor={item.endpoint.color}
														showValue={true} />
												</div>
											{:else}
												<span class="related-value-missing">-</span>
											{/if}
										</div>
									</div>
								{/each}
							</div>
						</section>
					{:else}
						<p class="empty-selection">No selection.</p>
					{/if}
				</div>
				<div
					class="endpoint-resize-handle"
					role="separator"
					aria-label="Resize preview panel"
					aria-orientation="vertical"
					onpointerdown={startInspectorResize}>
				</div>
			</aside>
			<button
				type="button"
				class="endpoint-show-tab"
				class:panel-visible={inspectorVisible}
				aria-hidden={inspectorVisible}
				title="Show preview panel"
				onclick={() => (inspectorVisible = true)}>
				<span class="endpoint-inspector-chevron">›</span>
				<span class="endpoint-inspector-label">Preview</span>
			</button>
		</div>
	{:else}
		<p class="missing">No Spatializer module found.</p>
	{/if}
</section>

<style>
	.edit-feedback {
		margin: 0;
		padding: 0.25rem 0.5rem;
		font-size: 0.8rem;
	}
	.edit-feedback:empty {
		display: none;
	}
	.spatializer-editor-panel {
		display: flex;
		flex-direction: column;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
		--spatializer-highlight-transition: 0.22s ease;
	}

	.editor-toolbar {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex: 0 0 auto;
		min-block-size: 2.4rem;
		padding: 0.35rem 0.55rem;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-text) 14%, transparent);
		background: color-mix(in srgb, var(--gc-color-background) 92%, var(--gc-color-text));
	}

	.module-picker {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		min-inline-size: 0;
		font-size: 0.78rem;
		color: color-mix(in srgb, var(--gc-color-text) 76%, transparent);
	}

	.module-picker select {
		max-inline-size: 18rem;
		min-inline-size: 10rem;
		padding: 0.25rem 0.45rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-text) 20%, transparent);
		border-radius: 0.25rem;
		background: var(--gc-color-panel, var(--gc-color-background));
		color: var(--gc-color-text);
		font: inherit;
	}

	.toolbar-chip {
		display: inline-flex;
		align-items: center;
		min-block-size: 1.45rem;
		padding: 0 0.45rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-accent) 35%, transparent);
		border-radius: 0.25rem;
		color: color-mix(in srgb, var(--gc-color-accent) 82%, var(--gc-color-text));
		font-size: 0.72rem;
		font-weight: 650;
	}

	.debug-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.32rem;
		margin-inline-start: auto;
		color: color-mix(in srgb, var(--gc-color-text) 72%, transparent);
		font-size: 0.72rem;
		font-weight: 600;
	}

	.debug-toggle input {
		inline-size: 0.9rem;
		block-size: 0.9rem;
		margin: 0;
		accent-color: var(--gc-color-accent);
	}

	.editor-body {
		position: relative;
		flex: 1 1 auto;
		inline-size: 100%;
		min-block-size: 0;
		min-inline-size: 0;
		overflow: hidden;
	}

	.canvas-region {
		position: absolute;
		inset: 0;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.endpoint-inspector {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		z-index: 30;
		display: grid;
		grid-template-rows: auto minmax(0, 1fr);
		min-inline-size: 0;
		max-inline-size: calc(100% - 2rem);
		min-block-size: 0;
		border-inline-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 72%, transparent);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		box-shadow: 0.5rem 0 2rem color-mix(in srgb, black 32%, transparent);
		transform: translateX(-100%);
		transition: transform 0.22s cubic-bezier(0.2, 0, 0.13, 1);
		pointer-events: none;
	}

	.endpoint-inspector.visible {
		transform: translateX(0);
		pointer-events: auto;
	}

	.endpoint-inspector-heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.28rem 0.35rem 0.28rem 0.5rem;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
	}

	.endpoint-inspector-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.18rem 0.2rem 0.18rem 0.3rem;
		border: none;
		border-radius: 0.3rem;
		background: transparent;
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
		transition: background 0.12s;
	}

	.endpoint-inspector-toggle:hover {
		background: color-mix(in srgb, var(--gc-color-accent, #66a6ff) 14%, transparent);
	}

	.endpoint-inspector-chevron {
		font-size: 0.9rem;
		line-height: 1;
		color: color-mix(in srgb, var(--gc-color-text) 65%, transparent);
	}

	.endpoint-inspector-label {
		font-size: 0.74rem;
		font-weight: 600;
	}

	.endpoint-inspector-body {
		display: flex;
		flex-direction: column;
		gap: 0.8rem;
		min-inline-size: 0;
		min-block-size: 0;
		padding: 0.75rem;
		overflow: auto;
	}

	.endpoint-show-tab {
		position: absolute;
		top: 0;
		left: 0;
		z-index: 35;
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.35rem 0.55rem 0.35rem 0.4rem;
		border: none;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		border-inline-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		border-end-end-radius: 0.4rem;
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 72%, transparent);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
		transition:
			opacity 0.18s,
			background 0.12s;
	}

	.endpoint-show-tab.panel-visible {
		opacity: 0;
		pointer-events: none;
	}

	.endpoint-show-tab:not(.panel-visible):hover {
		background: color-mix(
			in srgb,
			var(--gc-color-accent, #66a6ff) 18%,
			var(--gc-color-background-soft, #1a1a1a)
		);
	}

	.endpoint-resize-handle {
		position: absolute;
		inset-block: 0;
		inset-inline-end: 0;
		z-index: 5;
		inline-size: 0.35rem;
		cursor: col-resize;
		touch-action: none;
	}

	.endpoint-resize-handle:hover,
	.endpoint-resize-handle:active {
		background: color-mix(in srgb, var(--gc-color-accent, #66a6ff) 55%, transparent);
	}

	.endpoint-summary {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 0.5rem;
		min-inline-size: 0;
	}

	.endpoint-summary strong,
	.endpoint-summary span {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.endpoint-summary strong {
		font-size: 0.86rem;
	}

	.endpoint-summary span {
		font-size: 0.72rem;
		color: color-mix(in srgb, var(--gc-color-text) 64%, transparent);
	}

	.endpoint-swatch {
		inline-size: 0.9rem;
		block-size: 0.9rem;
		border-radius: 50%;
		box-shadow: 0 0 0 0.08rem color-mix(in srgb, var(--gc-color-text) 35%, transparent);
	}

	.related-values {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		min-block-size: 0;
	}

	.related-values header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.6rem;
		color: color-mix(in srgb, var(--gc-color-text) 64%, transparent);
		font-size: 0.72rem;
		font-weight: 650;
	}

	.related-value-list {
		display: flex;
		flex-direction: column;
		gap: 0.42rem;
		min-block-size: 0;
		overflow: visible;
	}

	.related-value-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 0.18rem;
		inline-size: 100%;
		min-inline-size: 0;
		padding: 0.18rem 0;
		border: 0.06rem solid transparent;
		border-radius: 0.25rem;
		background: transparent;
		color: var(--gc-color-text);
		font-size: 0.68rem;
	}

	.related-value-row.disabled {
		opacity: 0.48;
	}

	.related-value-label {
		display: inline-grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 0.38rem;
		inline-size: 100%;
		min-inline-size: 0;
		padding-inline: 0.08rem;
		color: color-mix(in srgb, var(--gc-color-text) 82%, transparent);
		font-weight: 550;
	}

	.related-value-label span:last-child {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.related-value-swatch {
		inline-size: 0.58rem;
		block-size: 0.58rem;
		border-radius: 50%;
		box-shadow: 0 0 0 0.06rem color-mix(in srgb, var(--gc-color-text) 28%, transparent);
	}

	.related-value-control {
		inline-size: 100%;
		min-inline-size: 0;
		overflow: hidden;
	}

	.related-value-slider {
		inline-size: 100%;
		min-inline-size: 0;
		block-size: 1rem;
	}

	.related-value-slider :global(.slider) {
		block-size: 100%;
		border-radius: 0.32rem;
	}

	.related-value-slider :global(.slider-label) {
		font-size: 0.62rem;
		font-weight: 650;
		color: var(--gc-color-text);
	}

	.related-value-missing {
		display: block;
		text-align: end;
		font-variant-numeric: tabular-nums;
		color: color-mix(in srgb, var(--gc-color-text) 52%, transparent);
	}

	.empty-selection,
	.missing {
		margin: 0;
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.8rem;
	}

	.missing {
		padding: 0.8rem;
	}

	@media (max-width: 54rem) {
		.endpoint-inspector {
			max-inline-size: calc(100% - 1.4rem);
		}
	}
</style>
