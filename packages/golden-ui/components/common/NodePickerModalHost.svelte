<script lang="ts">
	import { onDestroy, tick, untrack } from 'svelte';
	import { pickerCandidateAllowed, pickerProjectionAllowed } from './node-picker/selection';
	import { movePickerFocus } from './node-picker/focus';
	import { appState } from '../../store/workbench.svelte';
	import OutlinerItem from '../panels/outliner/OutlinerItem.svelte';
	import TreeRows from '../panels/outliner/TreeRows.svelte';
	import type { NodeSelectionInput } from '../panels/outliner/node-selection';
	import { createOutlinerProjection } from '../panels/outliner/visible-rows';
	import { createNodeOpenness } from '../panels/outliner/openness.svelte';
	import {
		closeNodePickerModal,
		type NodePickerModalView,
		nodePickerModalState,
		resolveNodePickerModal
	} from '../../store/node-picker-modal.svelte';
	import type { UiNodeDto, UiParamValueProjection } from '../../types';
	import { fade, slide } from 'svelte/transition';
	import {
		computeAnchoredModalPlacement,
		getMainViewportBounds,
		type ViewportRect
	} from './floating-panel';
	import { collectOutlinerAncestorNodeIds } from '../panels/outliner/navigation';

	let request = $derived(nodePickerModalState.request);
	let options = $derived(request?.options ?? null);
	let isOpen = $derived(options !== null);
	let query = $state('');
	let panelElement = $state<HTMLDivElement | null>(null);
	let searchInputElement = $state<HTMLInputElement | null>(null);
	let treeElement = $state<HTMLDivElement | null>(null);
	let virtualRows = $state<ReturnType<typeof TreeRows>>();
	export const navigateTree = (key: string, input?: Partial<NodeSelectionInput>) =>
		virtualRows?.navigate(key, input);
	export const enterTree = () => virtualRows?.enter();
	export const focusTreeNode = (id: number) => virtualRows?.focusNode(id);
	let topPx = $state(0);
	let leftPx = $state(0);
	let modalHeightPx = $state(0);
	let panelResizeObserver = $state<ResizeObserver | null>(null);
	let graphState = $derived(appState.session?.graph.state ?? null);
	let openedRequest = $state.raw<typeof request>(null);
	let ownerSession = $state.raw<typeof appState.session>(null);
	let returnFocus: HTMLElement | null = null;
	export const moveDialogFocus = (backwards = false) => movePickerFocus(panelElement, backwards);
	let opennessScope = $derived({
		request,
		session: appState.session,
		generation: appState.session?.snapshotGeneration
	});
	const openness = createNodeOpenness(
		() => graphState,
		() => opennessScope
	);
	export const setNodeExpanded = openness.setExpanded;
	let selectedNodeId = $state<number | null>(null);
	let selectedUuid = $state<string | undefined>();
	let autoExpandAncestorNodeIds = $derived.by(() =>
		collectOutlinerAncestorNodeIds(graphState, selectedNodeId)
	);
	let selectedProjection = $state<UiParamValueProjection | undefined>(undefined);
	let activeViewId = $state<string | null>(null);
	let treeIdentity = $derived({ request, activeViewId });
	let pickerViews = $derived(options?.views ?? []);
	let activeView = $derived.by((): NodePickerModalView | null => {
		const firstView = pickerViews[0] ?? null;
		if (activeViewId === null) {
			return firstView;
		}
		return pickerViews.find((view) => view.id === activeViewId) ?? firstView;
	});
	let showViewBar = $derived(pickerViews.length > 1);

	const getLiveAnchorRect = (): ViewportRect | null => {
		if (!options) {
			return null;
		}
		if (options.anchorElement && document.body.contains(options.anchorElement)) {
			const rect = options.anchorElement.getBoundingClientRect();
			return {
				left: rect.left,
				top: rect.top,
				right: rect.right,
				bottom: rect.bottom,
				width: rect.width,
				height: rect.height
			};
		}
		return options.anchorRect ?? null;
	};

	const recomputePosition = (): void => {
		if (!panelElement || !isOpen) {
			return;
		}
		const panelRect = panelElement.getBoundingClientRect();
		const placement = computeAnchoredModalPlacement({
			bounds: getMainViewportBounds(),
			panelWidth: panelRect.width,
			anchorRect: getLiveAnchorRect(),
			marginXRem: 0.75,
			marginYRem: 2,
			minHeightRem: 15,
			preferredHeightRem: 26
		});
		leftPx = placement.left;
		topPx = placement.top;
		modalHeightPx = placement.height;
	};

	const scrollToSelectedNode = (): void => {
		if (selectedNodeId !== null) virtualRows?.reveal(selectedNodeId);
	};
	$effect(() => {
		query;
		activeViewId;
		const target = selectedNodeId;
		if (isOpen && target !== null)
			void tick().then(() => {
				if (target === selectedNodeId) scrollToSelectedNode();
			});
	});
	export const searchNodes = (text: string): void => {
		query = text;
	};

	const nodeById = (nodeId: number | null): UiNodeDto | null => {
		if (nodeId === null || !graphState) {
			return null;
		}
		return graphState.nodesById.get(nodeId) ?? null;
	};

	const projectionOptionsForNode = (candidate: UiNodeDto | null): UiParamValueProjection[] => {
		if (!candidate || !options?.projectionOptions) {
			return [];
		}
		return options.projectionOptions(candidate) ?? [];
	};

	const projectionRequiredForNode = (candidate: UiNodeDto | null): boolean => {
		if (!candidate || !options?.projectionRequired) {
			return false;
		}
		return options.projectionRequired(candidate);
	};

	const projectionLabel = (projection: UiParamValueProjection): string => {
		return options?.projectionLabel?.(projection) ?? projection;
	};

	let selectedNode = $derived.by(() => {
		const node = nodeById(selectedNodeId);
		return node?.uuid === selectedUuid ? node : null;
	});
	let selectedProjectionOptions = $derived.by(() => projectionOptionsForNode(selectedNode));
	let selectedProjectionRequired = $derived.by(() => projectionRequiredForNode(selectedNode));
	let selectedProjectionChoiceCount = $derived.by(() => {
		if (!options?.projectionOptions || !selectedNode) {
			return 0;
		}
		return selectedProjectionOptions.length + (selectedProjectionRequired ? 0 : 1);
	});
	let showProjectionControls = $derived(
		Boolean(options?.projectionOptions && selectedProjectionChoiceCount > 1)
	);
	let canConfirmPick = $derived.by(() => {
		if (
			!selectedNode ||
			!options ||
			ownerSession !== appState.session ||
			!pickerCandidateAllowed(graphState, options, activeView, selectedNode)
		) {
			return false;
		}
		return pickerProjectionAllowed(options, selectedNode, selectedProjection);
	});

	const alignProjectionForNode = (candidate: UiNodeDto | null): void => {
		if (
			!options?.projectionOptions ||
			!candidate ||
			!pickerCandidateAllowed(graphState, options, activeView, candidate)
		) {
			selectedProjection = undefined;
			return;
		}
		const projectionOptions = projectionOptionsForNode(candidate);
		const projectionRequired = projectionRequiredForNode(candidate);
		if (selectedProjection !== undefined && projectionOptions.includes(selectedProjection)) {
			return;
		}
		if (projectionRequired && projectionOptions.length === 1) {
			selectedProjection = projectionOptions[0];
			return;
		}
		selectedProjection = undefined;
	};

	const resolvePick = (
		candidate: UiNodeDto,
		projection: UiParamValueProjection | undefined = undefined
	): boolean => {
		if (
			!options ||
			ownerSession !== appState.session ||
			openedRequest !== request ||
			!pickerCandidateAllowed(graphState, options, activeView, candidate) ||
			!pickerProjectionAllowed(options, candidate, projection)
		)
			return false;
		resolveNodePickerModal({
			kind: 'pick',
			node: candidate,
			projection
		});
		return true;
	};

	export const confirmPick = (): boolean => {
		if (!selectedNode || !canConfirmPick) {
			return false;
		}
		return resolvePick(selectedNode, selectedProjection);
	};
	export const chooseProjection = (projection: UiParamValueProjection | undefined): boolean => {
		if (
			!options ||
			!selectedNode ||
			!pickerCandidateAllowed(graphState, options, activeView, selectedNode) ||
			!pickerProjectionAllowed(options, selectedNode, projection)
		)
			return false;
		selectedProjection = projection;
		return true;
	};
	export const clearSelection = (): boolean => {
		if (!options?.clearable || ownerSession !== appState.session || openedRequest !== request)
			return false;
		resolveNodePickerModal({ kind: 'clear' });
		return true;
	};
	export const chooseView = (id: string): boolean => {
		if (!pickerViews.some((view) => view.id === id)) return false;
		activeViewId = id;
		return true;
	};
	export const activateNode = (id: number, confirm = false): boolean => {
		const candidate = nodeById(id);
		if (!candidate || !pickerCandidateAllowed(graphState, options, activeView, candidate))
			return false;
		if (selectedNodeId !== id || selectedUuid !== candidate.uuid) selectedProjection = undefined;
		selectedNodeId = id;
		selectedUuid = candidate.uuid;
		alignProjectionForNode(candidate);
		return !options?.projectionOptions ? resolvePick(candidate) : confirm ? confirmPick() : true;
	};

	$effect(() => {
		if (!isOpen) {
			untrack(() => {
				if (
					openedRequest &&
					returnFocus?.isConnected &&
					(panelElement?.contains(document.activeElement) ||
						document.activeElement === document.body)
				)
					returnFocus.focus({ preventScroll: true });
				returnFocus = null;
				openedRequest = null;
				ownerSession = null;
			});
			query = '';
			selectedNodeId = null;
			selectedUuid = undefined;
			selectedProjection = undefined;
			activeViewId = null;
			return;
		}
		untrack(() => {
			if (!openedRequest)
				returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
			openedRequest = request;
			ownerSession = appState.session;
		});
		query = options?.defaultSearchQuery ?? '';
		selectedNodeId = options?.selectedNodeId ?? null;
		selectedUuid = untrack(() => nodeById(selectedNodeId)?.uuid);
		selectedProjection = options?.selectedProjection ?? undefined;
		activeViewId = options?.views?.[0]?.id ?? null;
		let cancelled = false;
		void tick().then(() => {
			if (cancelled) return;
			searchInputElement?.focus();
			searchInputElement?.select();
			recomputePosition();
			scrollToSelectedNode();
		});
		return () => {
			cancelled = true;
		};
	});
	$effect(() => {
		const active = request,
			source = appState.session,
			graph = graphState,
			root = options?.rootNode;
		if (
			active &&
			openedRequest === active &&
			(ownerSession !== source || (root && graph?.nodesById.get(root.node_id)?.uuid !== root.uuid))
		)
			closeNodePickerModal();
	});

	$effect(() => {
		if (!isOpen) {
			return;
		}
		alignProjectionForNode(selectedNode);
	});

	$effect(() => {
		if (!isOpen || !panelElement || typeof ResizeObserver === 'undefined') {
			panelResizeObserver?.disconnect();
			panelResizeObserver = null;
			return;
		}

		const observer = new ResizeObserver(() => {
			recomputePosition();
		});
		observer.observe(panelElement);
		panelResizeObserver = observer;

		return () => {
			observer.disconnect();
			if (panelResizeObserver === observer) {
				panelResizeObserver = null;
			}
		};
	});

	const onWindowResize = (): void => {
		recomputePosition();
	};

	const onWindowScroll = (): void => {
		recomputePosition();
	};

	const onWindowKeydown = (event: KeyboardEvent): void => {
		if (!isOpen) {
			return;
		}
		if (event.key === 'Escape') {
			event.preventDefault();
			closeNodePickerModal();
		}
	};

	if (typeof window !== 'undefined') {
		window.addEventListener('resize', onWindowResize);
		window.addEventListener('scroll', onWindowScroll, true);
		window.addEventListener('keydown', onWindowKeydown);
	}

	onDestroy(() => {
		panelResizeObserver?.disconnect();
		panelResizeObserver = null;
		if (typeof window !== 'undefined') {
			window.removeEventListener('resize', onWindowResize);
			window.removeEventListener('scroll', onWindowScroll, true);
			window.removeEventListener('keydown', onWindowKeydown);
		}
	});

	const selectableByConstraints = (candidate: UiNodeDto): boolean => {
		if (!options) {
			return false;
		}
		return pickerCandidateAllowed(graphState, options, activeView, candidate);
	};

	const visibleByConstraints = (candidate: UiNodeDto): boolean => {
		if (!options) {
			return false;
		}
		if (!(options.nodeVisibilityFilter?.(candidate) ?? true)) {
			return false;
		}
		return activeView?.nodeVisibilityFilter?.(candidate) ?? true;
	};

	const rowVisibleByConstraints = (candidate: UiNodeDto): boolean => {
		return activeView?.nodeRowVisibilityFilter?.(candidate) ?? true;
	};

	const passesFilter = (candidate: UiNodeDto): boolean => {
		const normalizedQuery = query.trim().toLowerCase();
		return (
			visibleByConstraints(candidate) &&
			(!normalizedQuery ||
				(options?.nodeSearchText?.(candidate) ?? '').toLowerCase().includes(normalizedQuery))
		);
	};
	const projectRows = createOutlinerProjection();
	let rows = $derived.by(() => {
		const graph = graphState,
			config = options,
			view = activeView;
		if (!graph || !config?.rootNode) {
			projectRows.clear();
			return [];
		}
		const search = query.trim().toLowerCase(),
			searchText = config.nodeSearchText;
		const visible = config.nodeVisibilityFilter,
			viewVisible = view?.nodeVisibilityFilter;
		return projectRows({
			graph,
			roots: [{ nodeId: config.rootNode.node_id, parentId: null, level: 0, indent: 0 }],
			opennessByNodeId: openness.overrides,
			autoExpandAncestorNodeIds,
			initiallyExpandedDepth: config.initiallyExpandedDepth ?? 5,
			matchesQuery:
				search || visible || viewVisible
					? (node) =>
							(visible?.(node) ?? true) &&
							(viewVisible?.(node) ?? true) &&
							(!search || (searchText?.(node) ?? '').toLowerCase().includes(search))
					: undefined,
			rowVisible: view?.nodeRowVisibilityFilter
		});
	});
</script>

{#if options}
	<div
		transition:fade={{ duration: 100 }}
		class="node-picker-backdrop"
		role="button"
		tabindex="0"
		onclick={(event) => {
			if (event.target === event.currentTarget) {
				closeNodePickerModal();
			}
		}}
		onkeydown={(event) => {
			if (event.target !== event.currentTarget) {
				return;
			}
			if (event.key === 'Escape' || event.key === 'Enter' || event.key === ' ') {
				event.preventDefault();
				closeNodePickerModal();
			}
		}}>
		<div
			class="node-picker-modal"
			transition:slide={{ duration: 150 }}
			role="dialog"
			aria-modal="true"
			aria-label={options.title}
			onkeydown={(event) => {
				if (event.key === 'Tab' && !event.defaultPrevented) {
					event.preventDefault();
					event.stopPropagation();
					moveDialogFocus(event.shiftKey);
				}
			}}
			bind:this={panelElement}
			tabindex="-1"
			style={`top:${topPx}px;left:${leftPx}px;block-size:${modalHeightPx}px;max-block-size:${modalHeightPx}px;`}>
			<header class="node-picker-header">
				<h3>{options.title}</h3>
				<button type="button" class="close-button" onclick={() => closeNodePickerModal()}
					>Close</button>
			</header>
			<input
				type="search"
				class="picker-search"
				placeholder={options.searchPlaceholder}
				bind:this={searchInputElement}
				onkeydown={(event) => {
					if (event.key === 'ArrowDown') {
						event.preventDefault();
						void enterTree();
					}
				}}
				bind:value={query} />
			<div class="picker-view-bar" aria-label="Picker views">
				{#if showViewBar}
					{#each pickerViews as view}
						<button
							type="button"
							class:active={activeView?.id === view.id}
							aria-pressed={activeView?.id === view.id}
							onclick={() => chooseView(view.id)}>
							{view.label}
						</button>
					{/each}
				{/if}
			</div>
			<div
				class="picker-tree"
				bind:this={treeElement}
				role="tree"
				aria-label="Available nodes"
				tabindex="-1">
				{#if options.rootNode}
					<TreeRows
						bind:this={virtualRows}
						{rows}
						{selectedNodeId}
						identity={treeIdentity}
						onExpand={openness.setExpanded}
						selectable={selectableByConstraints}
						onActivate={(candidate) => {
							activateNode(candidate.node_id);
						}}
						variableHeights={true}
						scroller={treeElement}>
						{#snippet item(row)}
							<div class="picker-virtual-row" style:padding-inline-start={`${row.indent * 0.7}rem`}>
								<OutlinerItem
									nodeId={row.nodeId}
									siblingParentId={row.parentId}
									level={row.level}
									renderNodeChildren={false}
									projectedVisible={true}
									opennessByNodeId={openness.overrides}
									onNodeOpennessChange={openness.setExpanded}
									mode="tree"
									initiallyExpandedDepth={options.initiallyExpandedDepth}
									transitionDurationMs={90}
									focusedNodeId={selectedNodeId}
									{autoExpandAncestorNodeIds}
									nodeFilter={passesFilter}
									nodeRowVisible={rowVisibleByConstraints}
									nodeSelectable={selectableByConstraints}
									onSelectNode={(candidate, event) => {
										activateNode(candidate.node_id, event.detail >= 2);
									}} />
							</div>
						{/snippet}
					</TreeRows>
				{:else}
					<div class="picker-empty">No root available</div>
				{/if}
			</div>
			{#if showProjectionControls}
				<div class="projection-controls">
					<span class="projection-label">Projection</span>
					<select
						class="projection-select"
						disabled={!selectedNode ||
							!pickerCandidateAllowed(graphState, options, activeView, selectedNode)}
						value={selectedProjection ?? ''}
						onchange={(event) => {
							const value = (event.target as HTMLSelectElement).value;
							if (!value) chooseProjection(undefined);
							else {
								const projection = selectedProjectionOptions.find((option) => option === value);
								if (projection !== undefined) chooseProjection(projection);
							}
						}}>
						{#if !selectedProjectionRequired}
							<option value="">Auto</option>
						{/if}
						{#each selectedProjectionOptions as projection}
							<option value={projection}>{projectionLabel(projection)}</option>
						{/each}
					</select>
				</div>
			{/if}
			<footer class="picker-actions">
				{#if options.clearable}
					<button type="button" onclick={clearSelection}> Clear </button>
				{/if}
				{#if options.projectionOptions}
					<button type="button" disabled={!canConfirmPick} onclick={confirmPick}> Select </button>
				{/if}
			</footer>
		</div>
	</div>
{/if}

<style>
	.picker-virtual-row {
		inline-size: 100%;
		box-sizing: border-box;
	}
	.node-picker-backdrop {
		position: fixed;
		inset: 0;
		z-index: 1200;
		background: color-mix(in srgb, black 20%, transparent);
	}

	.node-picker-modal {
		position: fixed;
		inline-size: min(28rem, calc(100vw - 1.5rem));
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		padding: 0.65rem;
		border-radius: 0.45rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-focus) 35%, transparent);
		background: color-mix(in srgb, var(--gc-color-panel) 95%, black 5%);
		box-shadow: 0 0.6rem 1.2rem color-mix(in srgb, black 45%, transparent);
	}

	.node-picker-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
	}

	h3 {
		margin: 0;
		font-size: 0.85rem;
		font-weight: 700;
	}

	.close-button {
		padding: 0.15rem 0.45rem;
		font-size: 0.7rem;
	}

	.picker-search {
		inline-size: 100%;
		box-sizing: border-box;
	}

	.picker-view-bar {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		min-block-size: 1.45rem;
	}

	.picker-view-bar button {
		padding: 0.12rem 0.45rem;
		border-radius: 0.35rem;
		font-size: 0.72rem;
	}

	.picker-view-bar button.active {
		background: color-mix(in srgb, var(--gc-color-selection) 32%, transparent);
		outline: solid 0.06rem color-mix(in srgb, var(--gc-color-selection) 60%, transparent);
	}

	.picker-tree {
		flex: 1 1 auto;
		overflow: auto;
		padding-inline-end: 0.2rem;
	}

	.picker-empty {
		opacity: 0.72;
		font-size: 0.75rem;
	}

	.picker-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.4rem;
	}

	.projection-controls {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}

	.projection-label {
		font-size: 0.74rem;
		opacity: 0.75;
	}

	.projection-select {
		flex: 1 1 auto;
	}
</style>
