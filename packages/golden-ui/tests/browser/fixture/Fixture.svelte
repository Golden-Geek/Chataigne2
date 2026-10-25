<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import ManagerListPanel from '../../../components/panels/manager/ManagerListPanel.svelte';
	import OutlinerPanel from '../../../components/panels/outliner/OutlinerPanel.svelte';
	import NodePickerModalHost from '../../../components/common/NodePickerModalHost.svelte';
	import VirtualRows from '../../../components/common/virtual-rows/VirtualRows.svelte';
	import { appState } from '../../../store/app-state.svelte';
	import {
		openNodePickerModal,
		closeNodePickerModal
	} from '../../../store/node-picker-modal.svelte';
	import type { UiNodeDto } from '../../../types';
	import type { PanelApi, PanelParams } from '../../../dockview/panel-types';
	import { createFixtureSession } from './session';
	import RowSupplement from './RowSupplement.svelte';
	import CatalogFixture from './CatalogFixture.svelte';
	import LoggerFixture from './LoggerFixture.svelte';
	import { qualifyLoggerStore } from './logger-store';
	import { measureCollectionFlow } from './timing';
	const fixture = createFixtureSession();
	appState.session = fixture.session;
	onDestroy(() => {
		closeNodePickerModal();
		appState.session = null;
	});
	let count = 0;
	let query = $state('');
	let panelParams = $state.raw<PanelParams>({});
	const panelApi: PanelApi = {
		setTitle: () => {},
		close: () => {},
		getParams: <T extends PanelParams = PanelParams>() => panelParams as T,
		updateParams: (next) => {
			panelParams = { ...panelParams, ...next };
		}
	};
	let mode = $state<'trees' | 'variable' | 'catalog' | 'logger'>('trees');
	let logger = $state<ReturnType<typeof LoggerFixture>>();
	export async function setupLogger(size: number, unlimited = false, duplicates = false) {
		closeNodePickerModal();
		mode = 'logger';
		await tick();
		return logger!.setup(size, unlimited, duplicates);
	}
	export const logBrowse = (id: number) => logger!.browse(id);
	export const logFocus = (id: number) => logger!.focus(id);
	export const logSearch = (content: string, source = '', tag = '') =>
		logger!.search(content, source, tag);
	export const logAppend = () => logger!.append();
	export const logLive = () => logger!.live();
	export const logClear = () => logger!.clear();
	export const logCopyText = () => logger!.copyText();
	export const logCollapse = () => logger!.collapse();
	export const logSuperCollapse = () => logger!.superCollapse();
	export const measureLogs = () => logger!.measureFilter();
	export const measureLogAppend = () => logger!.measureAppend();
	export const logStorePublication = qualifyLoggerStore;
	let catalog = $state<ReturnType<typeof CatalogFixture>>();
	export async function setupCatalog(size: number) {
		closeNodePickerModal();
		mode = 'catalog';
		await tick();
		await catalog!.setup(size);
	}
	export const catalogReveal = (index: number, focus = false, depth = 0) =>
		catalog!.reveal(index, focus, depth);
	export const catalogSearch = (text: string, depth = 0) => catalog!.search(text, depth);
	export const catalogNested = () => catalog!.nested();
	export const catalogReverse = () => catalog!.reverse();
	export const catalogWidth = (width: string) => catalog!.setWidth(width);
	export const measureCatalog = () => catalog!.measureFilter();
	let manager = $state<ReturnType<typeof ManagerListPanel>>();
	let picker = $state<ReturnType<typeof NodePickerModalHost>>();
	let variable = $state<ReturnType<typeof VirtualRows<{ id: number; height: number }>>>();
	let variableScroller = $state<HTMLDivElement | null>(null);
	let variableRows = $state.raw<{ id: number; height: number }[]>([]);
	const treeNode = (node: UiNodeDto | null): node is UiNodeDto =>
		Boolean(node && ['FixtureItem', 'FixtureFolder'].includes(node.node_type));
	const frames = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
		await new Promise(requestAnimationFrame);
	};
	export async function setup(size: number) {
		appState.session = fixture.session;
		mode = 'trees';
		query = '';
		count = size;
		closeNodePickerModal();
		const value = fixture.load(size);
		await frames();
		return value;
	}
	export async function search(text: string) {
		query = text;
		await frames();
	}
	export async function select(id: number) {
		fixture.session.selectNode(id, 'REPLACE');
		await frames();
	}
	export async function expand(id: number, expanded: boolean, reject = false) {
		if (reject) fixture.rejectNext();
		manager!.setNodeExpanded(id, expanded);
		await frames();
	}
	export async function openPicker(selected: number) {
		void openNodePickerModal({
			rootNode: fixture.session.graph.state.nodesById.get(1) ?? null,
			selectedNodeId: selected,
			clearable: false
		});
		await frames();
	}
	export async function pickerSearch(text: string) {
		picker!.searchNodes(text);
		await frames();
	}
	export function lastItem() {
		return count + 1;
	}
	export async function setupVariable(size: number) {
		closeNodePickerModal();
		mode = 'variable';
		variableRows = Array.from({ length: size }, (_, id) => ({ id, height: id % 7 === 0 ? 4 : 2 }));
		await frames();
	}
	export async function revealVariable(id: number) {
		variable!.reveal(id, 'center');
		await frames();
	}
	export async function filterVariable(minimum: number) {
		variableRows = variableRows.filter((row) => row.id >= minimum);
		await frames();
	}
	const visible = (panel: string, nodeId: number, selected = false): boolean => {
		const root = document.querySelector<HTMLElement>(panel);
		const row = root?.querySelector<HTMLElement>(
			`.outliner-item-content[data-node-id="${nodeId}"]${selected ? '.selected' : ''}`
		);
		if (!root || !row) return false;
		const bounds = root.getBoundingClientRect(),
			rectangle = row.getBoundingClientRect();
		return rectangle.width > 0 && rectangle.bottom > bounds.top && rectangle.top < bounds.bottom;
	};
	export async function measureFlow(name: 'manager-filter' | 'selection' | 'picker-filter') {
		closeNodePickerModal();
		const closing = performance.now();
		while (document.querySelector('.node-picker-modal')) {
			if (performance.now() - closing > 5000) throw new Error('Picker did not finish closing.');
			await new Promise(requestAnimationFrame);
		}
		query = '';
		fixture.session.selectNode(2, 'REPLACE');
		await frames();
		if (name === 'picker-filter') await openPicker(count + 1);
		return measureCollectionFlow(
			name,
			count,
			(index) => {
				if (name === 'selection') fixture.session.selectNode(index % 2 ? 2 : count + 1, 'REPLACE');
				else {
					const text = index % 2 ? '' : `Item ${String(count - 1).padStart(6, '0')}`;
					if (name === 'picker-filter') picker!.searchNodes(text);
					else query = text;
				}
			},
			(index) => {
				if (name === 'selection') {
					const id = index % 2 ? 2 : count + 1;
					return (
						visible('.manager-list-content', id, true) && visible('.outliner-content', id, true)
					);
				}
				const panel = name === 'picker-filter' ? '.picker-tree' : '.manager-list-content';
				const total = Number(
					document.querySelector<HTMLElement>(`${panel} .virtual-rows`)?.dataset.rowCount
				);
				if (index % 2 ? total < count : total > 2) return false;
				return visible(panel, name === 'picker-filter' || !(index % 2) ? count + 1 : 2);
			}
		);
	}
</script>

<main>
	{#if mode === 'trees'}
		<section data-panel="manager">
			<ManagerListPanel
				bind:this={manager}
				bind:query
				managerNodeType="FixtureManager"
				rowSupplementComponent={RowSupplement}
				isTreeNode={treeNode}
				canRenderNodeChildren={(node) => node.node_type === 'FixtureFolder'} />
		</section>
		<section data-panel="outliner">
			<OutlinerPanel
				{panelApi}
				panelId="fixture"
				panelType="outliner"
				title="Project"
				params={panelParams} />
		</section>
	{:else if mode === 'catalog'}
		<CatalogFixture bind:this={catalog} />
	{:else if mode === 'logger'}
		<LoggerFixture bind:this={logger} {panelApi} />
	{:else}
		<div class="variable-scroller" bind:this={variableScroller} tabindex="-1">
			<VirtualRows
				bind:this={variable}
				rows={variableRows}
				keyOf={(row) => row.id}
				scroller={variableScroller}
				variableHeights={true}
				rowHeightRem={2}>
				{#snippet item(row)}<button data-variable-row={row.id} style:block-size={`${row.height}rem`}
						>Variable row {row.id}</button
					>{/snippet}
			</VirtualRows>
		</div>
	{/if}
	<NodePickerModalHost bind:this={picker} />
</main>

<style>
	main {
		display: flex;
		gap: 1rem;
		block-size: 36rem;
		inline-size: 70rem;
	}
	section {
		flex: 1;
		min-inline-size: 0;
		border: 0.06rem solid #666;
	}
	.variable-scroller {
		block-size: 32rem;
		inline-size: 30rem;
		overflow: auto;
	}
	button {
		display: block;
		inline-size: 100%;
		box-sizing: border-box;
	}
</style>
