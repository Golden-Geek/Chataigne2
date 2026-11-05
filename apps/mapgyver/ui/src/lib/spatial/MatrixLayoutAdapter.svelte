<script lang="ts">
	import { untrack } from 'svelte';
	import { appState, IconButton } from 'golden_ui';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import { createMatrixEditor } from '../matrix/editor.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import { SPATIAL_PAGE_SIZE } from './targets';
	import SpatialViewport from './SpatialViewport.svelte';
	import MatrixLayerControls from './MatrixLayerControls.svelte';
	import { createLayerEditor } from './layer-editor.svelte';
	let props: {
		panelId: string;
		media: string;
		document: string;
		present: boolean;
		locked: boolean;
		page: number;
		onpagechange(page: number): void;
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
	const rows = $derived(
		snapshot
			? selected && !snapshot.rows.some((row) => row.item.id.id === selected.item.id.id)
				? [...snapshot.rows, selected]
				: snapshot.rows
			: []
	);
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
	const pages = $derived(Math.max(1, Math.ceil((snapshot?.row_total ?? 0) / SPATIAL_PAGE_SIZE)));
	const selection = useDocumentSelection();
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
				identity={snapshot.final_output.reference.channel}
				label="Matrix Layer layout view"
				{actions}>
				<MatrixLayerControls
					{rows}
					{canvas}
					selected={selected?.item.id.id ?? null}
					disabled={owner.busy}
					select={(uuid) => selection.select(uuid)}
					active={geometry.state.active}
					start={geometry.start}
					move={(point) => geometry.gesture.move(point)}
					finish={() => geometry.gesture.finish()}
					stop={() => geometry.gesture.stop()} />
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
