<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import DashboardCanvas from '../../../components/panels/dashboard/DashboardCanvas.svelte';
	import {
		registerDashboardNodeWidgetType,
		unregisterDashboardNodeWidgetType
	} from '../../../components/panels/dashboard/dashboard-node-widget-registry';
	import type { DashboardLayoutKind } from '../../../components/panels/dashboard/dashboard-model';
	import { appState } from '../../../store/app-state.svelte';
	import { executeCommand } from '../../../store/commands.svelte';
	import { createFixtureSession } from './session';
	import { dashboardData } from './dashboard-data';
	import { dashboardForest } from './dashboard-forest';
	import { dashboardHierarchy } from './dashboard-hierarchy';
	import DashboardRegistryContent from './DashboardRegistryContent.svelte';
	import type { DashboardPersistedPageView } from '../../../components/panels/dashboard/canvas/contracts';
	import type { ParamValue } from '../../../types';
	import { getDirectParamNode } from '../../../components/panels/dashboard/dashboard-model';
	import type { FreeLayoutRect } from '../../../components/panels/dashboard/canvas/contracts';
	import type { SelectionMode } from '../../../store/workbench.svelte';
	const fixture = createFixtureSession();
	appState.session = fixture.session;
	registerDashboardNodeWidgetType({
		id: 'fixture',
		component: DashboardRegistryContent,
		usesLabelPlacement: true
	});
	onDestroy(() => {
		unregisterDashboardNodeWidgetType('fixture');
		appState.session = null;
	});
	let data = $state.raw<ReturnType<typeof dashboardData> | null>(null);
	let editMode = $state(false);
	let persisted = $state.raw<DashboardPersistedPageView | null>(null);
	let lastPersisted: DashboardPersistedPageView | null = null;
	let epoch = $state(0);
	let presentationBudget = $state(512);
	let canvas = $state<ReturnType<typeof DashboardCanvas>>();
	let sequence = 0;
	const settle = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
		await new Promise(requestAnimationFrame);
	};
	export async function setup(
		layout: DashboardLayoutKind,
		editing: boolean,
		sparseCount = 0,
		pattern: 'band' | 'grid' | 'dense' = 'band',
		nested = false
	) {
		data = null;
		await tick();
		editMode = editing;
		presentationBudget = 512;
		persisted = null;
		data = dashboardData(layout, sparseCount, pattern, nested);
		fixture.loadNodes(data.nodes);
		fixture.session.selectNodes([], 'REPLACE');
		epoch++;
		await settle();
		await settle();
		return { ids: data.ids, authoredNodes: data.nodes.length, containerId: data.containerId };
	}
	export async function setupTier(authoredNodes: number) {
		const widgets = Math.max(1, Math.ceil((authoredNodes - 11) / 11));
		return { ...(await setup('free', true, widgets, 'grid')), widgets };
	}
	export async function setupHierarchy(target: number, kind: 'free' | 'mixed' | 'deep') {
		data = null;
		await tick();
		editMode = true;
		persisted = null;
		presentationBudget = 512;
		const hierarchy = dashboardHierarchy(target, kind);
		data = hierarchy;
		fixture.loadNodes(hierarchy.nodes);
		fixture.session.selectNodes([], 'REPLACE');
		epoch++;
		await settle();
		await settle();
		return {
			ids: hierarchy.ids,
			containers: hierarchy.containers,
			authoredNodes: hierarchy.nodes.length
		};
	}
	export async function budget(value: number) {
		presentationBudget = value;
		await settle();
	}
	export async function chooseChild(surface: number, child: number) {
		const result = canvas!.chooseSurfaceChild(surface, child);
		await settle();
		return result;
	}
	export async function browse(surface: number) {
		const result = canvas!.browseSurface(surface);
		await settle();
		return result;
	}
	export async function searchChildren(surface: number, query: string) {
		canvas!.searchSurfaceChildren(surface, query);
		await settle();
	}
	export async function revealChild(surface: number, child: number) {
		const result = await canvas!.revealSurfaceChild(surface, child);
		await settle();
		return result;
	}
	export async function setupForest(count: number) {
		data = null;
		await tick();
		editMode = true;
		persisted = null;
		const forest = dashboardForest(count);
		data = forest;
		fixture.loadNodes(forest.nodes);
		fixture.session.selectNodes([], 'REPLACE');
		epoch++;
		await settle();
		await settle();
		return { ids: forest.ids, containers: forest.containers };
	}
	export async function selectAllWidgets() {
		fixture.session.selectNodes(data!.ids, 'REPLACE');
		await settle();
	}
	export function selected() {
		return fixture.session.selectedNodesIds;
	}
	export async function cancelFrameWithView() {
		document.querySelector<HTMLElement>('.dashboard-page-viewport')!.focus();
		await settle();
		const pending = executeCommand('view.frame', { source: 'api' });
		persisted = { panX: 17, panY: 19, zoom: 1 };
		const handled = await pending;
		await settle();
		return {
			handled,
			transform: document.querySelector<HTMLElement>('.dashboard-page-frame')!.style.transform
		};
	}
	export async function reveal(index: number) {
		const graph = fixture.session.graph.state;
		const item = graph.nodesById.get(data!.ids[index])!;
		const position = getDirectParamNode(graph, item, 'position')!;
		if (position.data.kind !== 'parameter' || position.data.param.value.kind !== 'vec2')
			throw new Error('Missing position');
		const layout = document.querySelector<HTMLElement>('.dashboard-page > .dashboard-layout')!;
		const viewport = document
			.querySelector<HTMLElement>('.dashboard-page-viewport')!
			.getBoundingClientRect();
		const rect = layout.getBoundingClientRect();
		const scale = rect.width / layout.clientWidth;
		const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
		const centerX = rect.left + (position.data.param.value.value[0] + 6 * rem) * scale;
		const centerY = rect.top + (position.data.param.value.value[1] + 2.5 * rem) * scale;
		persisted = {
			panX: (persisted?.panX ?? 0) + viewport.left + viewport.width / 2 - centerX,
			panY: (persisted?.panY ?? 0) + viewport.top + viewport.height / 2 - centerY,
			zoom: persisted?.zoom ?? 1
		};
		await settle();
	}
	export async function selectRegion(rect: FreeLayoutRect, mode: SelectionMode = 'REPLACE') {
		const selected = canvas!.selectWidgetsInRect(rect, mode);
		await settle();
		return selected;
	}
	export async function activate(index: number) {
		const graph = fixture.session.graph.state;
		const widget = graph.nodesById.get(data!.ids[index])!;
		const position = getDirectParamNode(graph, widget, 'position')!;
		if (position.data.kind !== 'parameter' || position.data.param.value.kind !== 'vec2')
			throw new Error('Expected authored widget position');
		const layout = document.querySelector<HTMLElement>('[data-dashboard-free-total]')!;
		const bounds = layout.getBoundingClientRect();
		const [x, y] = position.data.param.value.value;
		const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
		const id = canvas!.activateWidgetAt(
			data!.page.node_id,
			bounds.left + ((x + 6 * rem) * bounds.width) / layout.clientWidth,
			bounds.top + ((y + 2.5 * rem) * bounds.height) / layout.clientHeight
		);
		await settle();
		return id;
	}
	export async function focus(index: number) {
		const shell = document.querySelector<HTMLElement>(
			`.dashboard-widget-shell[data-node-id="${data!.ids[index]}"]`
		)!;
		const target =
			shell.querySelector<HTMLElement>('input:not(:disabled),textarea:not(:disabled)') ?? shell;
		target.focus();
		await settle();
		return data!.ids[index];
	}
	export async function rootFont(size: number) {
		document.documentElement.style.fontSize = `${size}px`;
		await settle();
	}
	export async function localFont(size: number) {
		document.querySelector<HTMLElement>('main')!.style.fontSize = `${size}px`;
		await settle();
	}
	export async function tab(index: number) {
		const applied = canvas!.activateSurfaceTab(data!.containerId!, data!.ids[index]);
		await settle();
		return applied;
	}
	export async function accordion(index: number) {
		const applied = canvas!.toggleSurfaceAccordion(data!.containerId!, data!.ids[index]);
		await settle();
		return applied;
	}
	export async function select(index: number) {
		fixture.session.selectNode(data!.ids[index], 'REPLACE');
		await settle();
	}
	export async function selectMany(indices: number[]) {
		fixture.session.selectNodes(
			indices.map((index) => data!.ids[index]),
			'REPLACE'
		);
		await settle();
	}
	export async function view(value: DashboardPersistedPageView) {
		persisted = value;
		await settle();
	}
	export async function update(index: number, key: string, value: ParamValue) {
		const graph = fixture.session.graph.state;
		const owner =
			index === -2
				? graph.nodesById.get(data!.containerId!)!
				: index < 0
					? data!.page
					: graph.nodesById.get(data!.ids[index])!;
		const node = getDirectParamNode(graph, owner, key);
		if (!node || node.data.kind !== 'parameter')
			throw new Error(`Missing fixture parameter ${key}`);
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: {
				kind: 'paramChanged',
				param: node.node_id,
				old_value: node.data.param.value,
				new_value: value
			}
		});
		await settle();
	}
	export async function enabled(index: number, key: string, value: boolean) {
		const graph = fixture.session.graph.state,
			owner = graph.nodesById.get(data!.ids[index])!;
		const field = getDirectParamNode(graph, owner, key);
		if (!field) throw new Error(`Missing fixture parameter ${key}`);
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: { kind: 'metaChanged', node: field.node_id, patch: { enabled: value } }
		});
		await settle();
	}
	export async function reorder(parent = data!.page.node_id) {
		const page = fixture.session.graph.state.nodesById.get(parent)!;
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: {
				kind: 'childReordered',
				parent: page.node_id,
				child: data!.ids[0],
				parent_children: [...page.children].reverse()
			}
		});
		await settle();
	}
	export async function remove(index: number) {
		const parent = fixture.session.graph.state.parentById.get(data!.ids[index])!;
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: { kind: 'childRemoved', parent, child: data!.ids[index] }
		});
		await settle();
	}
	export function placement(index: number) {
		const graph = fixture.session.graph.state;
		const owner = graph.nodesById.get(data!.ids[index])!;
		const node = getDirectParamNode(graph, owner, 'position')!;
		if (node.data.kind !== 'parameter') throw new Error('Missing position parameter');
		return node.data.param.value;
	}
	export async function command(id: 'view.home' | 'view.frame') {
		document.querySelector<HTMLElement>('.dashboard-page-viewport')!.focus();
		await settle();
		const handled = await executeCommand(id, { source: 'menu' });
		// The production camera animates for 220 ms. Observe completion across frames.
		const deadline = performance.now() + 300;
		while (performance.now() < deadline) await new Promise(requestAnimationFrame);
		await settle();
		return { handled, persisted: lastPersisted };
	}
	export async function frameWithReorder() {
		document.querySelector<HTMLElement>('.dashboard-page-viewport')!.focus();
		await settle();
		const element = document.querySelector<HTMLElement>('.dashboard-page-frame')!,
			before = element.style.transform;
		const pending = executeCommand('view.frame', { source: 'api' });
		await tick();
		await reorder(data!.containerId!);
		const handled = await pending;
		await settle();
		return { handled, before, after: element.style.transform };
	}
</script>

<main>
	{#if data}
		{#key epoch}
			<DashboardCanvas
				bind:this={canvas}
				node={data.page}
				{editMode}
				{presentationBudget}
				persistedPageView={persisted}
				onPersistedPageViewChange={(_nodeId, value) => {
					lastPersisted = value;
					persisted = value;
				}} />
		{/key}
	{/if}
</main>

<style>
	:global(html) {
		font-size: 16px;
	}
	:global(body) {
		margin: 0;
		font-family: sans-serif;
		background: #202020;
		color: #eee;
	}
	main {
		width: 90vw;
		height: 90vh;
		margin: 2rem;
	}
</style>
