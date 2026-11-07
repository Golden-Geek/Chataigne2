<script lang="ts">
	import { untrack } from 'svelte';
	import { appState, IconButton } from 'golden_ui';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import { createMatrixEditor } from '../matrix/editor.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import { SPATIAL_PAGE_SIZE } from './targets';
	import SpatialViewport from './SpatialViewport.svelte';
	import RectLayoutControls from './RectLayoutControls.svelte';
	import { createLayerEditor } from './layer-editor.svelte';
	import { matrixLayerPaintRows } from './rect-presentation';
	import type { RectGeometryMode } from '../media/generated/RectGeometryMode';
	import type { Point } from '../media/generated/Point';
	let props: {
		panelId: string;
		media: string;
		document: string;
		present: boolean;
		locked: boolean;
		page: number;
		onpagechange(page: number): void;
		onenterlayer(layer: string): void;
	} = $props();
	const owner = createMatrixEditor(() => ({
		selected: appState.session?.getSelectedNodes()[0]?.uuid ?? null,
		document: props.document,
		locked: props.media,
		retained: props.media,
		viewport: {
			row_start: props.page * SPATIAL_PAGE_SIZE,
			row_count: SPATIAL_PAGE_SIZE,
			axis_start: 0,
			axis_count: 1
		},
		selection: null,
		node: null
	}));
	untrack(() =>
		owner.controller.restore(
			{
				row_start: props.page * SPATIAL_PAGE_SIZE,
				row_count: SPATIAL_PAGE_SIZE,
				axis_start: 0,
				axis_count: 1
			},
			null,
			null
		)
	);
	$effect(() => {
		const page = props.page;
		untrack(() =>
			owner.controller.setViewport({
				row_start: page * SPATIAL_PAGE_SIZE,
				row_count: SPATIAL_PAGE_SIZE,
				axis_start: 0,
				axis_count: 1
			})
		);
	});
	const snapshot = $derived(owner.matrixWindow),
		source = $derived(snapshot?.final_output.definition.source);
	const canvas = $derived(
		source?.kind === 'Matrix' && source.output.kind === 'Final'
			? source.output.definition.canvas
			: null
	);
	const geometry = createLayerEditor(owner, () => props.present);
	const selected = $derived(
		(geometry.activeLayer
			? snapshot?.rows.find((row) => row.item.id.id === geometry.activeLayer)
			: null) ??
			snapshot?.selected?.row ??
			null
	);
	const rows = $derived(snapshot ? matrixLayerPaintRows(snapshot, selected) : []);
	const bounds = $derived(
		selected?.item.layer && canvas
			? {
					x: selected.item.layer.position[0] / canvas.width,
					y: selected.item.layer.position[1] / canvas.height,
					width: selected.item.layer.size.width / canvas.width,
					height: selected.item.layer.size.height / canvas.height
				}
			: null
	);
	const objects = $derived(
		rows.flatMap((row) =>
			row.item.layer
				? [
						{
							id: row.item.id.id,
							label: row.item.label,
							position: row.item.layer.position,
							size: row.item.layer.size,
							enabled: row.item.settings.enabled
						}
					]
				: []
		)
	);
	const pages = $derived(Math.max(1, Math.ceil((snapshot?.row_total ?? 0) / SPATIAL_PAGE_SIZE)));
	const selection = useDocumentSelection();
	function start(id: string, mode: RectGeometryMode, point: Point) {
		const object = objects.find((object) => object.id === id);
		return (
			!!object &&
			geometry.start(id, {
				origin: { position: [...object.position], size: { ...object.size } },
				mode,
				start: point
			})
		);
	}
</script>

<section
	class="workbench-editor"
	aria-label="Matrix layout"
	data-panel-id={props.panelId}
	data-spatial-kind="Matrix"
	data-spatial-media={props.present ? (snapshot?.media ?? '') : ''}
	data-spatial-level="Layers"
	data-spatial-selected={selected?.item.id.id ?? ''}
	data-composition-locked={props.locked}
	aria-busy={owner.busy || !props.present}>
	<div class="editor-canvas">
		{#if geometry.state.error || owner.view.error || owner.view.readError}<p
				class="editor-error"
				role="alert">
				{geometry.state.error || owner.view.error || owner.view.readError}
			</p>{/if}
		{#snippet actions()}
			{#if selected?.item.layer}
				<IconButton
					icon={arrowIcon}
					label="Edit Layer Cells"
					disabled={owner.busy}
					onclick={() => props.onenterlayer(selected!.item.id.id)} />
			{/if}
			{#if pages > 1}
				<IconButton
					class="previous"
					icon={arrowIcon}
					label="Previous Layers"
					disabled={owner.busy || props.page === 0}
					onclick={() => props.onpagechange(props.page - 1)} />
				<span>Page {props.page + 1} / {pages}</span>
				<IconButton
					icon={arrowIcon}
					label="Next Layers"
					disabled={owner.busy || props.page >= pages - 1}
					onclick={() => props.onpagechange(props.page + 1)} />
			{/if}
		{/snippet}
		{#if props.present && snapshot && canvas}
			<SpatialViewport
				channel={snapshot.final_output}
				{canvas}
				selection={bounds}
				keyboard={{
					id: selected?.item.id.id ?? null,
					disabled: owner.busy,
					active: geometry.state.active,
					start: (mode) => {
						const id = selected?.item.id.id;
						if (
							!id ||
							(typeof mode !== 'string' && 'Rotate' in mode) ||
							!start(id, mode, { x: 0, y: 0 })
						)
							return false;
						selection.select(id);
						return true;
					},
					move: (point) => geometry.gesture.move(point),
					finish: () => geometry.gesture.finish(),
					stop: () => geometry.gesture.stop()
				}}
				identity={snapshot.final_output.reference.channel}
				label="Matrix Layer layout view"
				{actions}>
				<RectLayoutControls
					{objects}
					kind="Layer"
					{canvas}
					selected={selected?.item.id.id ?? null}
					disabled={owner.busy}
					select={(uuid) => selection.select(uuid)}
					active={geometry.state.active}
					{start}
					move={(point) => geometry.gesture.move(point)}
					finish={() => geometry.gesture.finish()}
					stop={() => geometry.gesture.stop()}
					enter={props.onenterlayer} />
			</SpatialViewport>
		{:else if !props.present}<p class="editor-empty">
				{props.locked
					? 'Locked Matrix unavailable in this project.'
					: 'Matrix unavailable in this project.'}
			</p>
		{:else}<p class="editor-empty" role="status">Loading Matrix layout…</p>{/if}
	</div>
</section>

<style>
	:global(.previous img) {
		transform: rotate(180deg);
	}
</style>
