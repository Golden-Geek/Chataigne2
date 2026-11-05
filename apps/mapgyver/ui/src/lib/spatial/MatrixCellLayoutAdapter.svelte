<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState, IconButton, ViewportHandle } from 'golden_ui';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import { invokeMediaEdit } from '../media/operations';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	import type { MatrixLayoutCell } from '../media/generated/MatrixLayoutCell';
	import { useDocumentSelection } from '../selection.svelte';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import { CellLayoutReader, cellLayoutState } from './cell-layout';
	import { SPATIAL_PAGE_SIZE } from './targets';
	import SpatialViewport from './SpatialViewport.svelte';
	let props: {
		panelId: string;
		media: string;
		layer: string;
		present: boolean;
		locked: boolean;
		page: number;
		cell: string | null;
		onback(): void;
		onpage(page: number): void;
		oncell(cell: string | null): void;
		onlabel(label: string | null): void;
	} = $props();
	const state = $state(cellLayoutState());
	const reader = new CellLayoutReader(() => appState.session, state, invokeMediaEdit);
	const selection = useDocumentSelection(),
		preview = usePreview();
	$effect(() => {
		const session = appState.session;
		const query = {
			media: props.media,
			layer: props.layer,
			selected: props.cell,
			node: session?.getSelectedNodes()[0]?.uuid ?? null,
			start: props.page * SPATIAL_PAGE_SIZE,
			count: SPATIAL_PAGE_SIZE
		};
		if (session) {
			void session.status;
			void session.loading.visible;
			void session.projectGeneration;
			void session.snapshotGeneration;
			void session.currentHistoryStateId;
			void session.hasActiveEditSession;
			void session.editSessionEpoch;
			void session.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => reader.refresh(query));
	});
	const page = $derived(reader.page),
		layer = $derived(page?.row?.item.layer),
		selected = $derived(page?.selected);
	const cells = $derived(
		page
			? selected && !page.cells.some((cell) => cell.view.cell.id === selected.view.cell.id)
				? [...page.cells, selected]
				: page.cells
			: []
	);
	const pages = $derived(Math.max(1, Math.ceil((page?.total ?? 0) / SPATIAL_PAGE_SIZE)));
	function extent(cell: MatrixLayoutCell) {
		const reference = cell.view.cell.definition.source;
		return (
			cell.view.cell.definition.settings.size ??
			cell.extent ??
			(reference ? preview.statuses.get(previewKey({ kind: 'Channel', reference }))?.extent : null)
		);
	}
	const selectedExtent = $derived(selected ? extent(selected) : null);
	const bounds = $derived(
		selected && selectedExtent && layer
			? {
					x: selected.view.cell.definition.settings.position[0] / layer.size.width,
					y: selected.view.cell.definition.settings.position[1] / layer.size.height,
					width: selectedExtent.width / layer.size.width,
					height: selectedExtent.height / layer.size.height
				}
			: null
	);
	$effect(() => {
		const label = page?.row?.item.label ?? null;
		untrack(() => props.onlabel(label));
	});
	$effect(() => {
		if (page && page.selected_id !== props.cell) untrack(() => props.oncell(page.selected_id));
	});
	onDestroy(() => {
		reader.dispose();
		props.onlabel(null);
	});
	function select(cell: MatrixLayoutCell) {
		if (!reader.busy) {
			props.oncell(cell.view.cell.id);
			selection.select(cell.view.cell.id);
		}
	}
</script>

<section
	class="workbench-editor"
	aria-label="Matrix Cell layout"
	data-panel-id={props.panelId}
	data-spatial-kind="Matrix"
	data-spatial-media={props.present ? props.media : ''}
	data-spatial-level="Cells"
	data-spatial-layer={props.layer}
	data-spatial-selected={selected?.view.cell.id ?? ''}
	data-composition-locked={props.locked}
	aria-busy={reader.busy || !props.present}>
	<div class="editor-canvas">
		{#if state.error}<p class="editor-error" role="alert">{state.error}</p>{/if}
		{#snippet actions()}
			<IconButton
				class="previous"
				icon={arrowIcon}
				label="Matrix Layers"
				onclick={() => props.onback()} />
			<span>{page?.row?.item.label ?? 'Cells'}</span>
			{#if pages > 1}
				<IconButton
					class="previous"
					icon={arrowIcon}
					label="Previous Cells"
					disabled={reader.busy || props.page === 0}
					onclick={() => props.onpage(props.page - 1)} />
				<span>Page {props.page + 1} / {pages}</span>
				<IconButton
					icon={arrowIcon}
					label="Next Cells"
					disabled={reader.busy || props.page >= pages - 1}
					onclick={() => props.onpage(props.page + 1)} />
			{/if}
		{/snippet}
		{#if props.present && page?.context && layer}
			<SpatialViewport
				channel={page.context}
				canvas={layer.size}
				selection={bounds}
				identity={props.layer}
				label="Matrix Cell content view"
				preview={selected?.preview ?? null}
				previewLabel={selected?.label}
				{actions}>
				<svg
					viewBox={`0 0 ${layer.size.width} ${layer.size.height}`}
					preserveAspectRatio="none"
					role="group"
					aria-label="Matrix Cell layout controls">
					{#each cells as cell (cell.view.cell.id)}
						{@const size = extent(cell)}
						{#if size}
							<rect
								data-layout-cell={cell.view.cell.id}
								x={cell.view.cell.definition.settings.position[0]}
								y={cell.view.cell.definition.settings.position[1]}
								width={size.width}
								height={size.height}
								class:selected={selected?.view.cell.id === cell.view.cell.id}
								class:inactive={!cell.view.cell.definition.settings.enabled}
								role="button"
								tabindex={reader.busy ? -1 : 0}
								aria-label={`Select ${cell.label}`}
								aria-disabled={reader.busy}
								onclick={(event) => {
									event.stopPropagation();
									select(cell);
								}}
								onkeydown={(event) => {
									if (event.key === 'Enter' || event.key === ' ') {
										event.preventDefault();
										select(cell);
									}
								}} />
						{/if}
					{/each}
				</svg>
				{#each cells as cell (cell.view.cell.id)}
					{#if !extent(cell)}<ViewportHandle
							x={cell.view.cell.definition.settings.position[0] / layer.size.width}
							y={cell.view.cell.definition.settings.position[1] / layer.size.height}
							data-layout-cell={cell.view.cell.id}
							label={`Select ${cell.label}`}
							disabled={reader.busy}
							onclick={() => select(cell)} />{/if}
				{/each}
				{#if page.selected_id && !selected}<p class="cell-empty">Cell unavailable in this Layer.</p>
				{:else if !cells.length}<p class="cell-empty">No Cells in this Layer.</p>{/if}
			</SpatialViewport>
		{:else if page && !page.row}<p class="editor-empty">Layer unavailable in this project.</p>
			<div class="empty-actions">{@render actions()}</div>
		{:else}<p class="editor-empty" role="status">Loading Cell layout…</p>{/if}
	</div>
</section>

<style>
	:global(.previous img) {
		transform: rotate(180deg);
	}
	svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: visible;
		pointer-events: none;
	}
	rect {
		pointer-events: all;
		fill: transparent;
		stroke: var(--gc-color-border);
		stroke-width: 0.06rem;
		vector-effect: non-scaling-stroke;
		cursor: pointer;
	}
	rect.selected,
	rect:focus {
		stroke: var(--gc-color-focus);
		stroke-width: 0.12rem;
	}
	rect.inactive {
		stroke-dasharray: 0.3rem 0.2rem;
	}
	.cell-empty {
		position: absolute;
		inset: 0;
		display: grid;
		place-content: center;
		pointer-events: none;
	}
	.empty-actions {
		position: absolute;
		inset: 0 auto auto 0;
		display: flex;
		align-items: center;
		gap: 0.3rem;
	}
</style>
