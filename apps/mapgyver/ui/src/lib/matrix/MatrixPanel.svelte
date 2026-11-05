<script lang="ts">
	import {
		appState,
		readPanelPersistedState,
		writePanelPersistedState,
		ContextMenu,
		type ContextMenuAnchor,
		type ContextMenuItem,
		type PanelProps,
		type PanelState
	} from 'golden_ui';
	import { untrack } from 'svelte';
	import { createMatrixEditor } from './editor.svelte';
	import { matrixPanelState, type MatrixPanelState } from './panel-state';
	import { sameSelection } from './selection';
	import { sameViewport } from './viewport';
	import { useDocumentSelection } from '../selection.svelte';
	import VirtualGrid from './VirtualGrid.svelte';
	import { followPanelTitle } from '../panel-title.svelte';
	import { gridActions } from './grid-actions';
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
	import type { ColumnGroupSnapshot } from '../media/generated/ColumnGroupSnapshot';
	let props: PanelProps = $props();
	let choice = $state(
		untrack(() => matrixPanelState(readPanelPersistedState<MatrixPanelState>(props.params)))
	);
	export const setPanelState = (next: PanelState) => {
		choice = matrixPanelState(readPanelPersistedState<MatrixPanelState>(next.params));
		owner.controller.restore(choice.viewport, choice.selection, choice.node);
	};
	const owner = createMatrixEditor(() => ({
		selected: appState.session?.getSelectedNodes()[0]?.uuid ?? null,
		document: choice.document,
		locked: choice.locked ? choice.media : null,
		retained: choice.media,
		viewport: choice.viewport,
		selection: choice.selection,
		node: choice.node
	}));
	untrack(() => owner.controller.restore(choice.viewport, choice.selection, choice.node));
	const { view, controller } = owner;
	const matrixWindow = $derived(owner.matrixWindow),
		busy = $derived(owner.busy),
		observation = $derived(owner.observation);
	function persist() {
		writePanelPersistedState(props.panelApi, { ...choice });
	}
	$effect(() => {
		const page = view.page;
		if (!page?.snapshot || !page.document_matches || !page.media) return;
		const changed = choice.media !== page.media;
		if (
			!changed &&
			choice.document === page.document &&
			sameSelection(choice.selection, view.selected) &&
			choice.node === view.node &&
			sameViewport(choice.viewport, view.viewport)
		)
			return;
		choice = {
			...choice,
			document: page.document,
			media: page.media,
			selection: view.selected,
			node: view.node,
			viewport: { ...view.viewport },
			scroll: changed ? { top: 0, left: 0 } : choice.scroll
		};
		untrack(persist);
	});
	$effect(() => {
		props.panelApi.setContentBinding({
			locked: choice.locked,
			canLock: !!matrixWindow,
			toggle: () => {
				choice.locked = !choice.locked;
				persist();
			}
		});
	});
	followPanelTitle(
		() => props.panelApi,
		() => 'Matrix',
		() => matrixWindow?.label
	);
	const selection = useDocumentSelection();
	const selectDocumentNode = selection.select;
	let anchor = $state<ContextMenuAnchor | null>(null);
	let items = $state.raw<ContextMenuItem[]>([]);
	function menu(
		event: MouseEvent,
		row?: MatrixViewRow,
		column?: ColumnSnapshot,
		creationOnly = false,
		group?: ColumnGroupSnapshot
	) {
		if (busy) return;
		items = gridActions(owner, selection, row, column, creationOnly, group);
		anchor =
			event.type === 'click' && event.detail === 0 && event.currentTarget instanceof HTMLElement
				? { kind: 'element', element: event.currentTarget, placement: 'bottom-start' }
				: { kind: 'point', x: event.clientX, y: event.clientY };
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
	data-panel-id={props.panelId}
	data-matrix-target={matrixWindow?.media ?? ''}
	data-content-locked={choice.locked}
	aria-busy={busy}>
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
					applicationRuntime={owner.applicationRuntime}
					disabled={busy}
					viewport={(value) => controller.setViewport(value)}
					scroll={choice.media === view.media ? choice.scroll : { top: 0, left: 0 }}
					onscrollposition={(value, media) => {
						if (!matrixWindow || view.media !== media || choice.media !== media) return;
						if (choice.scroll.top === value.top && choice.scroll.left === value.left) return;
						choice.scroll = value;
						persist();
					}}
					{menu}
					createMenu={(event, axis) => {
						if (busy) return;
						items = gridActions(owner, selection, undefined, undefined, false, undefined, axis);
						anchor = {
							kind: 'element',
							element: event.currentTarget instanceof HTMLElement ? event.currentTarget : null,
							placement: 'bottom-start'
						};
					}}
					edit={(value) => owner.edit(value)}
					selectRow={(row) => {
						controller.select(row.item.id, view.selected?.column ?? null);
						selectDocumentNode(row.item.id.id);
					}}
					selectColumn={(column) => {
						controller.select(view.selected?.item ?? null, column.id);
						selectDocumentNode(column.id);
					}}
					selectedGroup={view.node}
					selectGroup={(group) => {
						controller.select(null, null);
						selectDocumentNode(group.id);
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
		{:else}<p class="editor-empty">
				{choice.locked
					? 'Locked Matrix unavailable in this project.'
					: 'Select a Matrix in Medias.'}
			</p>{/if}
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
