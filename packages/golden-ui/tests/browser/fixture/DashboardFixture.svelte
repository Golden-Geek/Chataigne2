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
	import { createDashboardInteractionChrome } from '../../../components/panels/dashboard/canvas/interaction-chrome.svelte';
	import {
		getDashboardPlacement,
		getDashboardLayoutKind
	} from '../../../components/panels/dashboard/dashboard-model';
	import type { UiClient } from '../../../types';
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
	type IntentReply = Awaited<ReturnType<UiClient['sendIntent']>>;
	const creations: {
		parent: number;
		target: number;
		resolve: ((reply: IntentReply) => void) | null;
		done: Promise<boolean> | null;
		settled: boolean;
	}[] = [];
	const waitingCreations: number[] = [];
	fixture.handleIntent((intent) => {
		if (intent.kind !== 'createDashboardNodeWidget') return null;
		return new Promise<IntentReply>((resolve) => {
			const request = waitingCreations.shift();
			if (request === undefined)
				throw new Error('Creation dispatched without a recorded operation');
			creations[request].resolve = resolve;
		});
	});
	export async function startCreation(index: number) {
		const graph = fixture.session.graph.state,
			source = graph.nodesById.get(data!.ids[index])!;
		const parent = data!.containerId!,
			owner = graph.nodesById.get(parent)!;
		const number = creations.length;
		const operation = {
			parent,
			target: source.node_id,
			resolve: null,
			done: null,
			settled: false
		} as (typeof creations)[number];
		creations.push(operation);
		waitingCreations.push(number);
		const done = canvas!.createWidgetOnSurface(parent, source.node_id, {
			label: `Create ${index}`,
			targetKind: 'node',
			placement: getDashboardPlacement(graph, source),
			targetIndex: getDashboardLayoutKind(graph, owner) === 'free' ? null : 3,
			genericWidgetKind: null,
			previewText: null,
			previewPlaceholder: null,
			multiline: false
		});
		operation.done = done;
		const finished = () => {
			operation.settled = true;
			const waiting = waitingCreations.indexOf(number);
			if (waiting >= 0) waitingCreations.splice(waiting, 1);
		};
		void done.then(finished, finished);
		await settle();
		return { request: operation.settled && !operation.resolve ? null : number };
	}
	export async function resolveCreation(request: number, success: boolean) {
		const pending = creations[request],
			id = 9000000 + request * 20;
		if (!pending.resolve) throw new Error(`Creation ${request} has not reached transport`);
		if (success) {
			const template = dashboardData('free');
			const widget = template.nodes.find((node) => node.node_type === 'dashboard_node_widget')!;
			const source = fixture.session.graph.state.nodesById.get(pending.target)!;
			const subtree = structuredClone([
				widget,
				...widget.children.map((child) => template.nodes.find((node) => node.node_id === child)!)
			]);
			const ids = new Map(subtree.map((node, index) => [node.node_id, id + index]));
			for (const node of subtree) {
				node.node_id = ids.get(node.node_id)!;
				node.uuid = `created-${node.node_id}`;
				node.children = node.children.map((child) => ids.get(child)!);
				if (node.data.kind === 'parameter') {
					if (node.data.param.value.kind === 'reference') {
						node.data.param.value = {
							...node.data.param.value,
							uuid: source.uuid,
							cached_id: source.node_id
						};
						node.data.param.default_value = node.data.param.value;
					}
					if (node.decl_id === 'position') node.data.param.value = { kind: 'vec2', value: [0, 0] };
				} else node.meta.label = `Created ${request}`;
				fixture.session.graph.applyEvent({
					time: { tick: ++sequence, micro: 0, seq: 0 },
					kind: { kind: 'nodeCreated', node: node.node_id, snapshot: node }
				});
			}
			fixture.session.graph.applyEvent({
				time: { tick: ++sequence, micro: 0, seq: 0 },
				kind: {
					kind: 'childAdded',
					parent: pending.parent,
					child: id,
					decl_id: '',
					parent_children: [
						...fixture.session.graph.state.nodesById.get(pending.parent)!.children,
						id
					]
				}
			});
		}
		pending.resolve(
			success
				? { success: true, status: 'applied' }
				: { success: false, status: 'rejected', error_message: 'Fixture creation rejection' }
		);
		const result = await pending.done;
		await settle();
		return { result, id };
	}
	let moveSource = $state<number | null>(null);
	let moveCancellations: (() => void)[] = [];
	const moveChrome = createDashboardInteractionChrome({
		get node() {
			return fixture.session.graph.state.nodesById.get(moveSource!)!;
		},
		get parent() {
			const graph = fixture.session.graph.state;
			return graph.nodesById.get(graph.parentById.get(moveSource!)!) ?? null;
		},
		content: {
			widgetKind: 'text',
			genericDisplayValue: '',
			textConfig: '',
			placeholderConfig: '',
			multiline: false
		}
	});
	export async function movePreview(
		index: number | null,
		targetIndex: number | null = 0,
		commit = false
	) {
		if (index === null) {
			moveChrome.setFreeLayoutAncestorShells(null);
			moveChrome.publishWidgetMoveSurfacePreview(null);
			moveChrome.publishWidgetMoveCommit(null);
		} else {
			moveSource = data!.ids[index];
			moveChrome.setFreeLayoutAncestorShells(
				document.querySelector<HTMLElement>(`.dashboard-widget-shell[data-node-id="${moveSource}"]`)
			);
			const graph = fixture.session.graph.state;
			const target = {
				surfaceNodeId: data!.containerId!,
				targetIndex,
				placement: getDashboardPlacement(graph, graph.nodesById.get(moveSource)!)
			};
			if (commit) {
				const cancel = moveChrome.publishWidgetMoveCommit(
					target,
					graph.parentById.get(moveSource) ?? null
				);
				if (cancel) moveCancellations.push(cancel);
			} else moveChrome.publishWidgetMoveSurfacePreview(target);
		}
		await settle();
	}
	export async function cancelMoveCommit(index: number) {
		moveCancellations[index]();
		await settle();
	}
	export async function reparent(index: number) {
		const graph = fixture.session.graph.state,
			child = data!.ids[index];
		const old_parent = graph.parentById.get(child)!,
			new_parent = data!.page.node_id;
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: {
				kind: 'childMoved',
				child,
				old_parent,
				new_parent,
				old_parent_children: graph.nodesById.get(old_parent)!.children.filter((id) => id !== child),
				new_parent_children: [...graph.nodesById.get(new_parent)!.children, child]
			}
		});
		await settle();
	}
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
		moveCancellations = [];
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
	export async function editing(value: boolean) {
		editMode = value;
		await settle();
	}
	export async function refreshIdentities() {
		fixture.loadNodes(structuredClone([...fixture.session.graph.state.nodesById.values()]));
		await settle();
	}
	export async function acknowledgeMove(index: number, targetIndex: number) {
		const graph = fixture.session.graph.state;
		const id = data!.ids[index],
			parent = graph.parentById.get(id)!;
		const owner = graph.nodesById.get(parent)!;
		const widgets = owner.children.filter(
			(child) => graph.nodesById.get(child)?.user_role === 'itemRoot' && child !== id
		);
		widgets.splice(targetIndex, 0, id);
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: {
				kind: 'childReordered',
				parent,
				child: id,
				parent_children: [
					...owner.children.filter((child) => graph.nodesById.get(child)?.user_role !== 'itemRoot'),
					...widgets
				]
			}
		});
		await settle();
		return fixture.session.graph.state.nodesById
			.get(parent)!
			.children.filter((child) => graph.nodesById.get(child)?.user_role === 'itemRoot')
			.slice(0, 5);
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
	export async function selectSurface() {
		fixture.session.selectNode(data!.containerId!, 'REPLACE');
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
