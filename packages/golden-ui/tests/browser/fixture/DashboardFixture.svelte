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
	import DashboardRegistryContent from './DashboardRegistryContent.svelte';
	import type { DashboardPersistedPageView } from '../../../components/panels/dashboard/canvas/contracts';
	import type { ParamValue } from '../../../types';
	import { getDirectParamNode } from '../../../components/panels/dashboard/dashboard-model';
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
	let sequence = 0;
	const settle = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
		await new Promise(requestAnimationFrame);
	};
	export async function setup(layout: DashboardLayoutKind, editing: boolean, sparseCount = 0) {
		data = null;
		await tick();
		editMode = editing;
		persisted = null;
		data = dashboardData(layout, sparseCount);
		fixture.loadNodes(data.nodes);
		epoch++;
		await settle();
		return { ids: data.ids, authoredNodes: data.nodes.length };
	}
	export async function select(index: number) {
		fixture.session.selectNode(data!.ids[index], 'REPLACE');
		await settle();
	}
	export async function view(value: DashboardPersistedPageView) {
		persisted = value;
		await settle();
	}
	export async function update(index: number, key: string, value: ParamValue) {
		const graph = fixture.session.graph.state;
		const owner = index < 0 ? data!.page : graph.nodesById.get(data!.ids[index])!;
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
	export async function reorder() {
		const page = fixture.session.graph.state.nodesById.get(data!.page.node_id)!;
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
</script>

<main>
	{#if data}
		{#key epoch}
			<DashboardCanvas
				node={data.page}
				{editMode}
				persistedPageView={persisted}
				onPersistedPageViewChange={(_nodeId, value) => {
					lastPersisted = value;
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
