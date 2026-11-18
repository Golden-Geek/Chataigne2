<script lang="ts">
	import {
		appState,
		nodeSelectionMode,
		readPanelPersistedState,
		writePanelPersistedState,
		ContextMenu,
		IconButton,
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
	import NodeColorMenu from 'golden_ui/components/common/NodeColorMenu.svelte';
	import { nodeColorMenuItems } from 'golden_ui/components/common/node-color-menu.ts';
	import { matrixColorTarget } from './color-target';
	import { presetActions } from './preset-actions';
	import settingsIcon from 'golden_ui/style/icons/settings.svg';
	import { createCellTransfers, type CellDrag } from './cell-transfer';
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
		selected: appState.session?.selectedNodeUuid ?? null,
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
	const transfers = createCellTransfers({
		session: () => owner.session,
		target: () =>
			owner.matrixWindow
				? { project: owner.matrixWindow.project, media: owner.matrixWindow.media }
				: null,
		busy: () => owner.busy,
		edit: (edit) => owner.edit(edit),
		captureSelection: selection.capture,
		selectAddress: (layer, column) => controller.select({ kind: 'Layer', id: layer }, column)
	});
	const selectDocumentNode = selection.select;
	let anchor = $state<ContextMenuAnchor | null>(null);
	let items = $state.raw<ContextMenuItem[]>([]);
	let colors = $state<ReturnType<typeof NodeColorMenu>>();
	async function inspectMembers(
		preset: string,
		after: string | null = null,
		previous: (string | null)[] = []
	) {
		const session = owner.session,
			generation = session?.snapshotGeneration,
			media = view.media,
			opened = anchor;
		const result = await owner.edit({
			kind: 'Preset',
			edit: { kind: 'InspectMembers', preset, after, count: 32 }
		});
		if (
			!anchor ||
			anchor !== opened ||
			owner.session !== session ||
			session?.snapshotGeneration !== generation ||
			view.media !== media ||
			result?.kind !== 'MatrixPresetMembers'
		)
			return;
		const page = result.page;
		items = [
			{
				label: 'Configurations',
				closeOnSelect: false,
				action: () => {
					items = presetActions(owner, selection, inspectMembers);
				}
			},
			{ separator: true },
			...page.members.map((member): ContextMenuItem => ({
				label: member.label,
				action: () => {
					selection.select(member.id);
				}
			})),
			...(page.members.length ? [] : [{ label: 'No captured members', disabled: true }]),
			...(previous.length
				? [
						{
							label: 'Previous members',
							closeOnSelect: false,
							action: () => inspectMembers(preset, previous.at(-1)!, previous.slice(0, -1))
						}
					]
				: []),
			...(page.next
				? [
						{
							label: 'Next members',
							closeOnSelect: false,
							action: () => inspectMembers(preset, page.next, [...previous, after])
						}
					]
				: [])
		];
	}
	function menu(
		event: MouseEvent,
		row?: MatrixViewRow,
		column?: ColumnSnapshot,
		creationOnly = false,
		group?: ColumnGroupSnapshot
	) {
		if (busy) return;
		items = gridActions(
			owner,
			selection,
			row,
			column,
			creationOnly,
			group,
			undefined,
			transfers,
			(address) => {
				const target = matrixColorTarget(owner, address);
				return target
					? nodeColorMenuItems(
							target,
							(target, point) => colors?.open(target, point),
							() => {
								anchor = null;
								items = [];
							}
						)
					: [];
			}
		);
		anchor =
			event.type === 'click' && event.detail === 0 && event.currentTarget instanceof HTMLElement
				? { kind: 'element', element: event.currentTarget, placement: 'bottom-start' }
				: { kind: 'point', x: event.clientX, y: event.clientY };
	}
	function cellDrop(source: CellDrag, layer: string, column: string, event: DragEvent) {
		if (busy) return;
		if (event.altKey) {
			items = transfers.menu(source, layer, column);
			anchor = { kind: 'point', x: event.clientX, y: event.clientY };
		} else void transfers.run(source, layer, column, 'Drop');
	}
	$effect(() => {
		void view.media;
		void owner.session?.snapshotGeneration;
		anchor = null;
		items = [];
	});
</script>

<NodeColorMenu bind:this={colors} />

<section
	class="workbench-editor"
	aria-label={props.title ?? 'Matrix'}
	data-panel-id={props.panelId}
	data-matrix-target={matrixWindow?.media ?? ''}
	data-content-locked={choice.locked}
	aria-busy={busy}>
	{#if matrixWindow}
		<div class="editor-toolbar">
			<IconButton
				icon={settingsIcon}
				label="Matrix configurations"
				disabled={busy}
				onclick={(event) => {
					items = presetActions(owner, selection, inspectMembers);
					anchor = { kind: 'element', element: event.currentTarget, placement: 'bottom-start' };
				}} />
		</div>
	{/if}
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
					observationReceivedAt={owner.observationReceivedAt}
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
					cellSource={transfers.source}
					{cellDrop}
					cellCopy={transfers.copy}
					cellPaste={transfers.paste}
					createMenu={(event, axis) => {
						if (busy) return;
						items = gridActions(
							owner,
							selection,
							undefined,
							undefined,
							false,
							undefined,
							axis,
							transfers
						);
						anchor = {
							kind: 'element',
							element: event.currentTarget instanceof HTMLElement ? event.currentTarget : null,
							placement: 'bottom-start'
						};
					}}
					edit={(value) => owner.edit(value)}
					selectRow={(row, event) => {
						controller.select(row.item.id, view.selected?.column ?? null, row.item.id.id);
						selectDocumentNode(row.item.id.id, nodeSelectionMode(event));
					}}
					selectColumn={(column, event) => {
						controller.select(view.selected?.item ?? null, column.id, column.id);
						selectDocumentNode(column.id, nodeSelectionMode(event));
					}}
					clearSelection={() => {
						if (!matrixWindow) return;
						controller.select(null, null, null);
						selectDocumentNode(matrixWindow.media);
					}}
					selectGroup={(group, event) => {
						controller.select(null, null, group.id);
						selectDocumentNode(group.id, nodeSelectionMode(event));
					}}
					select={(row, column, event) => {
						const cell = matrixWindow?.cells.find(
							(cell) =>
								cell.cell.layer === row.item.id.id && cell.cell.definition.column === column.id
						);
						controller.select(row.item.id, column.id, cell?.cell.id ?? row.item.id.id);
						selectDocumentNode(cell?.cell.id ?? row.item.id.id, nodeSelectionMode(event));
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
							controller.select(row.item.id, column.id, result.cell);
							selectDocumentNode(result.cell);
						}
					}}
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
		items = [];
	}} />

<style>
	.matrix-canvas {
		display: flex;
		flex-direction: column;
	}
</style>
