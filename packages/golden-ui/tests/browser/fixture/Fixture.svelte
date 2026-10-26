<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import ManagerListPanel from '../../../components/panels/manager/ManagerListPanel.svelte';
	import OutlinerPanel from '../../../components/panels/outliner/OutlinerPanel.svelte';
	import NodePickerModalHost from '../../../components/common/NodePickerModalHost.svelte';
	import VirtualRows from '../../../components/common/virtual-rows/VirtualRows.svelte';
	import { appState } from '../../../store/app-state.svelte';
	import {
		openNodePickerModal,
		closeNodePickerModal,
		type NodePickerModalResult
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
	let outliner = $state<ReturnType<typeof OutlinerPanel>>();
	let picker = $state<ReturnType<typeof NodePickerModalHost>>();
	let constrainedBlock = $state(false);
	let constrainedResults: (NodePickerModalResult | null)[] = [];
	export async function openConstrained(
		selected = 3,
		mode: 'multiple' | 'single' | 'optional' | 'instant' | 'invalid' = 'multiple'
	) {
		constrainedBlock = false;
		const slot = constrainedResults.length;
		constrainedResults.push(null);
		void openNodePickerModal({
			rootNode: fixture.session.graph.state.nodesById.get(1)!,
			selectedNodeId: selected,
			selectedProjection: mode === 'invalid' ? 'floatToVec2XX' : undefined,
			nodeFilter: (node) =>
				node.node_type === 'FixtureItem' &&
				node.node_id !== 3 &&
				(!constrainedBlock || node.node_id !== 4),
			views: [
				{ id: 'all', label: 'All', nodeRowVisibilityFilter: (node) => node.node_id !== 1 },
				{
					id: 'even',
					label: 'Even',
					nodeVisibilityFilter: (node) => node.node_id % 2 === 0,
					nodeRowVisibilityFilter: (node) => node.node_id !== 1 && node.node_id !== 2
				},
				{ id: 'none', label: 'None', nodeVisibilityFilter: () => false }
			],
			projectionOptions:
				mode === 'instant'
					? undefined
					: (node) => (mode === 'single' || node.node_id !== 4 ? ['vec2X'] : ['vec2X', 'vec2Y']),
			projectionRequired: () => mode !== 'optional'
		}).then((result) => {
			constrainedResults[slot] = result;
		});
		await frames();
		return slot;
	}
	export const pickerResults = () =>
		constrainedResults.map((result) =>
			result?.kind === 'pick'
				? {
						kind: result.kind,
						id: result.node.node_id,
						uuid: result.node.uuid,
						projection: result.projection
					}
				: result
		);
	export async function constrainedCommand(command: string, value = 0) {
		let accepted: boolean | undefined;
		if (command === 'confirm') accepted = picker!.confirmPick();
		else if (command === 'clear') accepted = picker!.clearSelection();
		else if (command === 'activate') accepted = picker!.activateNode(value);
		else if (command === 'double') accepted = picker!.activateNode(value, true);
		else if (command === 'x') accepted = picker!.chooseProjection('vec2X');
		else if (command === 'y') accepted = picker!.chooseProjection('vec2Y');
		else if (command === 'auto') accepted = picker!.chooseProjection(undefined);
		else if (command === 'all' || command === 'even' || command === 'none')
			accepted = picker!.chooseView(command);
		else if (command === 'next' || command === 'previous')
			accepted = picker!.moveDialogFocus(command === 'previous');
		else if (command === 'block') constrainedBlock = true;
		else if (command === 'close') closeNodePickerModal();
		else if (command === 'replace-node') {
			const node = fixture.session.graph.state.nodesById.get(value)!;
			fixture.session.graph.applyEvent({
				time: { tick: 9500, micro: 0, seq: 0 },
				kind: {
					kind: 'nodeCreated',
					node: value,
					snapshot: { ...node, uuid: `${node.uuid}-replaced` }
				}
			});
		} else if (command === 'replace-root') fixture.load(count);
		else throw new Error(`Unknown constrained picker command ${command}`);
		await frames();
		return accepted;
	}
	export async function setupConstrained(size: number) {
		await setup(size);
		constrainedResults = [];
		document.querySelector<HTMLInputElement>('.manager-list-search')!.focus();
	}
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
		panelParams = {};
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
	export async function openPicker(selected: number, projection = false) {
		void openNodePickerModal({
			rootNode: fixture.session.graph.state.nodesById.get(1) ?? null,
			selectedNodeId: selected,
			clearable: false,
			projectionOptions: projection ? () => [] : undefined
		});
		await frames();
	}
	type TreePanel = 'manager' | 'outliner' | 'picker';
	const treePanel = (panel: TreePanel) =>
		panel === 'manager' ? manager! : panel === 'outliner' ? outliner! : picker!;
	export async function treeFocus(panel: TreePanel, id: number) {
		const result = await treePanel(panel).focusTreeNode(id);
		await frames();
		return result;
	}
	export async function treeNavigate(panel: TreePanel, key: string) {
		await treePanel(panel).navigateTree(key);
		await frames();
	}
	export const treeSelection = () => [...fixture.session.selectedNodesIds];
	export async function treeModifiers(panel: TreePanel) {
		fixture.session.selectNode(2, 'REPLACE');
		await treePanel(panel).focusTreeNode(3);
		// Browser events expose modifiers through prototype getters.
		await treePanel(panel).navigateTree(
			' ',
			Object.create({
				get ctrlKey() {
					return true;
				}
			})
		);
		await frames();
		const toggled = [...fixture.session.selectedNodesIds];
		await treePanel(panel).navigateTree(' ', { altKey: true });
		await frames();
		return { toggled, removed: [...fixture.session.selectedNodesIds] };
	}
	export async function treeSearch(panel: TreePanel, text: string) {
		if (panel === 'manager') query = text;
		else if (panel === 'outliner') outliner!.searchNodes(text);
		else picker!.searchNodes(text);
		await frames();
	}
	export async function treeCollapse(panel: TreePanel, id: number) {
		treePanel(panel).setNodeExpanded(id, false);
		await frames();
	}
	export async function treeDelete(id: number) {
		fixture.session.graph.applyEvent({
			time: { tick: 9000, micro: 0, seq: 0 },
			kind: { kind: 'nodeDeleted', node: id }
		});
		await frames();
	}
	export async function treeRace(panel: TreePanel, action: 'newer' | 'search' | 'replace') {
		const component = treePanel(panel);
		const pending = component.focusTreeNode(count + 1);
		let newer: Promise<boolean | undefined> | undefined;
		if (action === 'newer') newer = component.focusTreeNode(3);
		else if (action === 'search') {
			const selector =
				panel === 'manager'
					? '.manager-list-search'
					: panel === 'outliner'
						? '.outliner-search'
						: '.picker-search';
			document.querySelector<HTMLInputElement>(selector)!.focus();
		} else fixture.load(count);
		const results = [await pending, newer ? await newer : undefined];
		await frames();
		if (panel === 'picker' && action === 'replace') {
			const deadline = performance.now() + 5000;
			while (document.querySelector('.node-picker-modal')) {
				if (performance.now() > deadline) throw new Error('Replaced picker did not retire.');
				await new Promise(requestAnimationFrame);
			}
		}
		return results;
	}
	export async function treeEnter(panel: TreePanel) {
		await treePanel(panel).enterTree();
		await frames();
	}
	export async function treeReject(panel: TreePanel) {
		fixture.rejectNext();
		await treeNavigate(panel, 'ArrowRight');
	}
	export async function treeMeasure(panel: TreePanel, action: 'focus' | 'expand') {
		const component = treePanel(panel);
		await component.focusTreeNode(2);
		await frames();
		const root =
			panel === 'manager'
				? '.manager-list-content'
				: panel === 'outliner'
					? '.outliner-content'
					: '.picker-tree';
		if (action === 'expand') {
			await component.navigateTree('ArrowRight');
			await frames();
		}
		return measureCollectionFlow(
			`tree-${panel}-${action}`,
			count,
			(index) => {
				void component.navigateTree(
					action === 'focus' ? (index % 2 ? 'Home' : 'End') : index % 2 ? 'ArrowRight' : 'ArrowLeft'
				);
			},
			(index) => {
				const target =
					action === 'focus'
						? index % 2
							? panel === 'manager'
								? 2
								: panel === 'outliner'
									? 0
									: 1
							: count + 1
						: 2;
				const row = document.querySelector<HTMLElement>(`${root} [data-tree-node-id="${target}"]`);
				if (!row || row !== document.activeElement || !visible(root, target)) return false;
				return (
					action !== 'expand' || row.getAttribute('aria-expanded') === String(Boolean(index % 2))
				);
			}
		);
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
				bind:this={outliner}
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
