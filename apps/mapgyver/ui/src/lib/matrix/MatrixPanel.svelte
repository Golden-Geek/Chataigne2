<script lang="ts">
	import { IconButton, type PanelProps, type PanelState } from 'golden_ui';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import groupIcon from 'golden_ui/style/icons/node/folder.svg';
	import resetIcon from 'golden_ui/style/icons/reset.svg';
	import stopIcon from 'golden_ui/style/icons/delete.svg';
	import { useMatrixAuthoring } from './context.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	import VirtualGrid from './VirtualGrid.svelte';
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	const owner = useMatrixAuthoring();
	const { view, controller } = owner;
	const matrixWindow = $derived(owner.matrixWindow),
		busy = $derived(owner.busy),
		observation = $derived(owner.observation);
	const { select: selectDocumentNode } = useDocumentSelection();
</script>

<section
	class="workbench-editor"
	aria-label={props.title ?? 'Matrix'}
	data-panel-id={props.panelId}>
	<div class="editor-toolbar" aria-label="Matrix actions">
		<span>{matrixWindow?.final_output.label ?? 'Matrix'}</span>
		<IconButton
			icon={addIcon}
			label="Add Matrix"
			disabled={busy}
			onclick={() => controller.create()} />
		<IconButton
			icon={resetIcon}
			label="Refresh Matrix"
			disabled={busy}
			onclick={() => controller.observe()} />
		{#if matrixWindow}
			<IconButton
				icon={addIcon}
				label="Add layer"
				disabled={busy}
				onclick={() => controller.edit({ kind: 'AddLayers', parent: null, count: 1 })} />
			<IconButton
				icon={groupIcon}
				label="Add group"
				disabled={busy}
				onclick={() => controller.edit({ kind: 'AddGroup', parent: null, label: null })} />
			<IconButton
				icon={addIcon}
				label="Add column"
				disabled={busy}
				onclick={() => controller.edit({ kind: 'AddColumns', count: 1 })} />
			<IconButton
				icon={stopIcon}
				label="Unload Matrix"
				disabled={busy || matrixWindow.playback.desired_column === null}
				onclick={() => controller.edit({ kind: 'ClearPlayback' })} />
		{/if}
	</div>
	{#if view.error || view.readError}<p class="editor-error" role="alert">
			{view.error || view.readError}
		</p>{/if}
	<div class="editor-canvas matrix-canvas">
		{#if matrixWindow}
			{#key view.media}
				<VirtualGrid
					snapshot={matrixWindow}
					selected={view.selected}
					{observation}
					disabled={busy}
					viewport={(value) => controller.setViewport(value)}
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
						selectDocumentNode(row.item.id.id);
					}}
					launch={(column) =>
						controller.edit({ kind: 'LaunchColumn', column, mode: 'Configured' })} />
			{/key}
		{:else}<p class="editor-empty">Select a Matrix in Media Manager.</p>{/if}
	</div>
</section>

<style>
	.matrix-canvas {
		display: flex;
		flex-direction: column;
	}
</style>
