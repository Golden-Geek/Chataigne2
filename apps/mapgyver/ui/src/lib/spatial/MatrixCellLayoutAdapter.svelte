<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { observeEditor } from 'golden_ui/store/editor-observation.svelte';
	import { usePanelVisibility } from 'golden_ui/dockview/panel-visibility.svelte';
	import { projectCellLayoutPage } from './parameter-projection';
	import { appState, IconButton } from 'golden_ui';
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
	import RectLayoutControls from './RectLayoutControls.svelte';
	import { cellExtent } from './rect-presentation';
	import { createCellEditor } from './cell-editor.svelte';
	import type { RectGeometryMode } from '../media/generated/RectGeometryMode';
	import type { Point } from '../media/generated/Point';
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
	const panel = usePanelVisibility();
	const reader = new CellLayoutReader(() => appState.session, state, invokeMediaEdit);
	const geometry = createCellEditor(reader, () => props.present);
	const selection = useDocumentSelection(),
		preview = usePreview();
	observeEditor({
		query: () => ({
			media: props.media,
			layer: props.layer,
			selected: props.cell,
			node: appState.session?.selectedNodeUuid ?? null,
			start: props.page * SPATIAL_PAGE_SIZE,
			count: SPATIAL_PAGE_SIZE
		}),
		topics: [MEDIA_NAMESPACE],
		owners: () => (reader.page ? [reader.page.layer] : []),
		interacting: () => state.interacting,
		refresh: (query, resolve) => reader.refresh(query, resolve)
	});
	const parameters = $derived(
		panel.visible && reader.page ? geometry.projection.read(state.generation, reader.page.at) : null
	);
	const page = $derived(
			reader.page && parameters && !state.interacting
				? projectCellLayoutPage(reader.page, parameters)
				: reader.page
		),
		layer = $derived(page?.row?.item.layer),
		selected = $derived(
			(geometry.activeCell
				? page?.cells.find((cell) => cell.view.cell.id === geometry.activeCell)
				: null) ?? page?.selected
		);
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
		return cellExtent(
			cell,
			reference ? preview.statuses.get(previewKey({ kind: 'Channel', reference }))?.extent : null
		);
	}
	const objects = $derived(
		cells.map((cell) => ({
			id: cell.view.cell.id,
			label: cell.label,
			position: cell.view.cell.definition.settings.position,
			size: extent(cell),
			enabled: cell.view.cell.definition.settings.enabled
		}))
	);
	const selectedExtent = $derived(selected ? extent(selected) : null);
	const bounds = $derived(
		selected && layer
			? selectedExtent
				? {
						x: selected.view.cell.definition.settings.position[0] / layer.size.width,
						y: selected.view.cell.definition.settings.position[1] / layer.size.height,
						width: selectedExtent.width / layer.size.width,
						height: selectedExtent.height / layer.size.height
					}
				: {
						x: selected.view.cell.definition.settings.position[0] / layer.size.width - 0.05,
						y: selected.view.cell.definition.settings.position[1] / layer.size.height - 0.05,
						width: 0.1,
						height: 0.1
					}
			: null
	);
	$effect(() => {
		const label = page?.row?.item.label ?? null;
		untrack(() => props.onlabel(label));
	});
	$effect(() => {
		const selectedId = page?.selected_id;
		if (page)
			untrack(() => {
				if (selectedId !== props.cell) props.oncell(selectedId ?? null);
			});
	});
	onDestroy(() => {
		reader.dispose();
		props.onlabel(null);
	});
	function select(id: string) {
		props.oncell(id);
		selection.select(id);
	}
	function start(id: string, mode: RectGeometryMode, point: Point) {
		const object = objects.find((object) => object.id === id);
		if (!object) return false;
		if (mode === 'Translate')
			return geometry.start(id, { kind: 'Translate', origin: [...object.position], start: point });
		return (
			!!object.size &&
			geometry.start(id, {
				kind: 'Resize',
				origin: { position: [...object.position], size: { ...object.size } },
				handle: mode.Resize.handle,
				start: point
			})
		);
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
		{#if state.error || geometry.state.error}<p class="editor-error" role="alert">
				{state.error || geometry.state.error}
			</p>{/if}
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
				keyboard={{
					id: selected?.view.cell.id ?? null,
					disabled: reader.busy,
					active: geometry.state.active,
					start: (mode) => {
						const id = selected?.view.cell.id;
						if (
							!id ||
							(typeof mode !== 'string' && 'Rotate' in mode) ||
							!start(id, mode, { x: 0, y: 0 })
						)
							return false;
						select(id);
						return true;
					},
					move: (point) => geometry.gesture.move(point),
					finish: () => geometry.gesture.finish(),
					stop: () => geometry.gesture.stop()
				}}
				identity={props.layer}
				label="Matrix Cell content view"
				preview={selected?.preview ?? null}
				previewLabel={selected?.label}
				{actions}>
				<RectLayoutControls
					{objects}
					presentation={geometry.presentation}
					kind="Cell"
					canvas={layer.size}
					selected={selected?.view.cell.id ?? null}
					disabled={reader.busy}
					active={geometry.state.active}
					{select}
					{start}
					move={(point) => geometry.gesture.move(point)}
					finish={() => geometry.gesture.finish()}
					stop={() => geometry.gesture.stop()} />
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
