<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { appState } from '../../store/workbench.svelte';
	import OutlinerItem from '../panels/outliner/OutlinerItem.svelte';
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
	import {
		collectOutlinerAncestorNodeIds,
		scrollOutlinerNodeIntoView
	} from '../panels/outliner/navigation';

	let request = $derived(nodePickerModalState.request);
	let options = $derived(request?.options ?? null);
	let isOpen = $derived(options !== null);
	let query = $state('');
	let panelElement = $state<HTMLDivElement | null>(null);
	let searchInputElement = $state<HTMLInputElement | null>(null);
	let treeElement = $state<HTMLDivElement | null>(null);
	let topPx = $state(0);
	let leftPx = $state(0);
	let modalHeightPx = $state(0);
	let panelResizeObserver = $state<ResizeObserver | null>(null);
	let graphState = $derived(appState.session?.graph.state ?? null);
	let selectedNodeId = $state<number | null>(null);
	let autoExpandAncestorNodeIds = $derived.by(() =>
		collectOutlinerAncestorNodeIds(graphState, selectedNodeId)
	);
	let selectedProjection = $state<UiParamValueProjection | undefined>(undefined);
	let activeViewId = $state<string | null>(null);
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
		scrollOutlinerNodeIntoView(treeElement, selectedNodeId);
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

	let selectedNode = $derived.by(() => nodeById(selectedNodeId));
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
		if (!selectedNode) {
			return false;
		}
		if (!options?.projectionOptions) {
			return true;
		}
		if (!selectedProjectionRequired) {
			return true;
		}
		return selectedProjection !== undefined;
	});

	const alignProjectionForNode = (candidate: UiNodeDto | null): void => {
		if (!options?.projectionOptions || !candidate) {
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

	const autoProjectionForNode = (candidate: UiNodeDto): UiParamValueProjection | undefined => {
		if (!options?.projectionOptions) {
			return undefined;
		}
		if (!projectionRequiredForNode(candidate)) {
			return undefined;
		}
		return projectionOptionsForNode(candidate)[0];
	};

	const resolvePick = (
		candidate: UiNodeDto,
		projection: UiParamValueProjection | undefined = autoProjectionForNode(candidate)
	): void => {
		resolveNodePickerModal({
			kind: 'pick',
			node: candidate,
			projection
		});
	};

	const confirmPick = (): void => {
		if (!selectedNode || !canConfirmPick) {
			return;
		}
		resolvePick(selectedNode, selectedProjection);
	};

	$effect(() => {
		if (!isOpen) {
			query = '';
			selectedNodeId = null;
			selectedProjection = undefined;
			activeViewId = null;
			return;
		}
		query = options?.defaultSearchQuery ?? '';
		selectedNodeId = options?.selectedNodeId ?? null;
		selectedProjection = options?.selectedProjection ?? undefined;
		activeViewId = options?.views?.[0]?.id ?? null;
		void tick().then(() => {
			searchInputElement?.focus();
			searchInputElement?.select();
			recomputePosition();
			scrollToSelectedNode();
			requestAnimationFrame(() => {
				searchInputElement?.focus();
				recomputePosition();
				scrollToSelectedNode();
			});
			window.setTimeout(() => {
				scrollToSelectedNode();
			}, 120);
		});
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
		return options.nodeFilter?.(candidate) ?? true;
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

	const searchMatchByNodeId = $derived.by(() => {
		const directMatches = new Map<number, boolean>();
		const subtreeMatches = new Map<number, boolean>();
		if (!options?.rootNode || !graphState) {
			return subtreeMatches;
		}

		const normalizedQuery = query.trim().toLowerCase();
		const visitOrder: number[] = [];
		const pending: number[] = [options.rootNode.node_id];
		while (pending.length > 0) {
			const nodeId = pending.pop();
			if (nodeId === undefined) {
				continue;
			}
			const node = graphState.nodesById.get(nodeId);
			if (!node) {
				continue;
			}
			visitOrder.push(nodeId);
			let isMatch = visibleByConstraints(node);
			if (isMatch && normalizedQuery.length > 0) {
				const text = options.nodeSearchText?.(node) ?? '';
				isMatch = text.toLowerCase().includes(normalizedQuery);
			}
			directMatches.set(nodeId, isMatch);
			for (const childId of node.children) {
				pending.push(childId);
			}
		}

		for (let index = visitOrder.length - 1; index >= 0; index -= 1) {
			const nodeId = visitOrder[index];
			const node = graphState.nodesById.get(nodeId);
			if (!node) {
				continue;
			}
			let isVisible = directMatches.get(nodeId) ?? false;
			if (!isVisible) {
				for (const childId of node.children) {
					if (subtreeMatches.get(childId) ?? false) {
						isVisible = true;
						break;
					}
				}
			}
			subtreeMatches.set(nodeId, isVisible);
		}

		return subtreeMatches;
	});

	const passesFilter = (candidate: UiNodeDto): boolean => {
		return searchMatchByNodeId.get(candidate.node_id) ?? false;
	};
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
				bind:value={query} />
			<div class="picker-view-bar" aria-label="Picker views">
				{#if showViewBar}
					{#each pickerViews as view}
						<button
							type="button"
							class:active={activeView?.id === view.id}
							aria-pressed={activeView?.id === view.id}
							onclick={() => {
								activeViewId = view.id;
							}}>
							{view.label}
						</button>
					{/each}
				{/if}
			</div>
			<div class="picker-tree" bind:this={treeElement}>
				{#if options.rootNode}
					<OutlinerItem
						node={options.rootNode}
						mode="tree"
						initiallyExpandedDepth={options.initiallyExpandedDepth}
						transitionDurationMs={90}
						focusedNodeId={selectedNodeId}
						{autoExpandAncestorNodeIds}
						nodeFilter={passesFilter}
						nodeRowVisible={rowVisibleByConstraints}
						nodeSelectable={selectableByConstraints}
						onSelectNode={(candidate, event) => {
							if (event.detail >= 2) {
								resolvePick(candidate);
								return;
							}
							if (!options.projectionOptions) {
								resolvePick(candidate);
								return;
							}
							selectedNodeId = candidate.node_id;
							alignProjectionForNode(candidate);
						}} />
				{:else}
					<div class="picker-empty">No root available</div>
				{/if}
			</div>
			{#if showProjectionControls}
				<div class="projection-controls">
					<span class="projection-label">Projection</span>
					<select
						class="projection-select"
						disabled={!selectedNode}
						value={selectedProjection ?? ''}
						onchange={(event) => {
							const value = (event.target as HTMLSelectElement).value;
							selectedProjection = value.length > 0 ? (value as UiParamValueProjection) : undefined;
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
					<button type="button" onclick={() => resolveNodePickerModal({ kind: 'clear' })}>
						Clear
					</button>
				{/if}
				{#if options.projectionOptions}
					<button type="button" disabled={!canConfirmPick} onclick={confirmPick}> Select </button>
				{/if}
			</footer>
		</div>
	</div>
{/if}

<style>
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
