<script lang="ts">
	import {
		ContextMenu,
		type ContextMenuAnchor,
		type ContextMenuItem,
		type PanelProps,
		type PanelState
	} from 'golden_ui';
	import { useMatrixAuthoring } from './context.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import VirtualGrid from './VirtualGrid.svelte';
	import { gridActions } from './grid-actions';
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	const owner = useMatrixAuthoring();
	const { view, controller } = owner;
	const matrixWindow = $derived(owner.matrixWindow),
		busy = $derived(owner.busy),
		observation = $derived(owner.observation);
	const selection = useDocumentSelection();
	const selectDocumentNode = selection.select;
	let anchor = $state<ContextMenuAnchor | null>(null);
	let items = $state.raw<ContextMenuItem[]>([]);
	function menu(
		event: MouseEvent,
		row?: MatrixViewRow,
		column?: ColumnSnapshot,
		creationOnly = false
	) {
		if (busy) return;
		items = gridActions(owner, selection, row, column, creationOnly);
		anchor = { kind: 'point', x: event.clientX, y: event.clientY };
	}
	$effect(() => {
		void view.media;
		void owner.session?.snapshotGeneration;
		anchor = null;
	});
</script>

<section
	class="workbench-editor"
	aria-label={props.title ?? 'Matrix'}
	data-panel-id={props.panelId}>
	<div class="editor-canvas matrix-canvas">
		{#if view.error || view.readError}<p class="editor-error" role="alert">
				{view.error || view.readError}
			</p>{/if}
		{#if matrixWindow}
			{#key view.media}
				<VirtualGrid
					snapshot={matrixWindow}
					selected={view.selected}
					{observation}
					disabled={busy}
					viewport={(value) => controller.setViewport(value)}
					{menu}
					edit={(value) => owner.edit(value)}
					selectRow={(row) => {
						controller.select(row.item.id, view.selected?.column ?? null);
						selectDocumentNode(row.item.id.id);
					}}
					selectColumn={(column) => {
						controller.select(view.selected?.item ?? null, column.id);
						selectDocumentNode(column.id);
					}}
					select={(row, column) => {
						controller.select(row.item.id, column.id);
						const cell = matrixWindow?.cells.find(
							(cell) =>
								cell.cell.layer === row.item.id.id && cell.cell.definition.column === column.id
						);
						selectDocumentNode(cell?.cell.id ?? row.item.id.id);
					}}
					drop={async (row, column, source, copy) => {
						if (row.item.id.kind !== 'Layer') return;
						const result = await owner.edit({
							kind: 'DropSource',
							layer: row.item.id.id,
							column: column.id,
							source,
							copy
						});
						if (result?.kind === 'MatrixMediaCreated') {
							controller.select(row.item.id, column.id);
							selectDocumentNode(result.cell);
						}
					}}
					launch={(column) => owner.edit({ kind: 'LaunchColumn', column, mode: 'Configured' })}
					trigger={(row, column) => {
						if (row.item.id.kind === 'Layer')
							void owner.edit({
								kind: 'LaunchCell',
								layer: row.item.id.id,
								column: column.id,
								mode: 'Configured'
							});
					}} />
			{/key}
		{:else}<p class="editor-empty">Select a Matrix in Media Manager.</p>{/if}
	</div>
</section>
<ContextMenu
	open={anchor !== null}
	{anchor}
	{items}
	onClose={() => {
		anchor = null;
	}} />

<style>
	.matrix-canvas {
		display: flex;
		flex-direction: column;
	}
</style>
