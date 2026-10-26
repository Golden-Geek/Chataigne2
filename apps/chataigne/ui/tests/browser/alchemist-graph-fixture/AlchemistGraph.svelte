<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { appState, createWorkbenchSession } from 'golden_ui/store/workbench.svelte';
	import type { PanelApi, PanelParams, PanelProps, UiNodeDto } from 'golden_ui';
	import { wholeGraphScope } from 'golden_ui/types';
	import { createDefaultUiClient } from 'golden_ui';
	import type { UiReplayCursor } from 'golden_ui/types';
	import AlchemistEditorPanel from '../../../src/lib/systems/alchemist/components/AlchemistEditorPanel.svelte';
	import { createNativeGraphMeasurement } from '../app-graph-scale/measurement';
	import { createNativeFormulaConnections } from '../app-graph-scale/formula-connections';
	import { anodeType } from '../../../src/lib/systems/alchemist/alchemistGraph';
	const base = new URLSearchParams(location.search).get('backend');
	if (!base) throw new Error('Missing owned native backend URL');
	let holdNextSnapshot = false;
	let heldSnapshot = $state<UiReplayCursor | null>(null);
	let releaseSnapshot: (() => void) | null = null;
	let heldRefresh: Promise<boolean> | null = null;
	let latestSnapshot = $state<UiReplayCursor | null>(null);
	const session = createWorkbenchSession({
		httpBaseUrl: `${base}/api/ui`,
		wsUrl: `${base.replace('http:', 'ws:')}/api/ui/ws`,
		enableGlobalShortcuts: false,
		transportFactory: (options) => {
			const client = createDefaultUiClient(options);
			return {
				...client,
				async snapshot(scope) {
					const result = await client.snapshot(scope);
					const cursor = { project_generation: result.project_generation, at: { ...result.at } };
					if (holdNextSnapshot) {
						holdNextSnapshot = false;
						heldSnapshot = cursor;
						await new Promise<void>((resolve) => {
							releaseSnapshot = resolve;
						});
						heldSnapshot = null;
					}
					latestSnapshot = cursor;
					return result;
				}
			};
		}
	});
	appState.session = session;
	onMount(() => session.mount());
	let panel = $state<ReturnType<typeof AlchemistEditorPanel>>();
	let mounted = $state(true);
	let params = $state.raw<PanelParams>({ camera: { x: 0, y: 0, zoom: 1 } });
	const panelApi: PanelApi = {
		setTitle() {},
		close() {},
		getParams: <T extends PanelParams = PanelParams>() => params as T,
		updateParams: (next) => {
			params = { ...params, ...next };
		}
	};
	let props = $derived({
		panelApi,
		panelId: 'native-formula',
		panelType: 'AlchemistEditor',
		title: 'Native Formula',
		params
	} satisfies PanelProps);
	const frames = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
	};
	const idle = async () => {
		const deadline = performance.now() + 10000;
		while (session.editOperations.size || session.userActionPendingCount) {
			if (performance.now() > deadline) throw new Error('Native graph operation did not settle');
			await frames();
		}
		await frames();
	};
	const child = (owner: UiNodeDto, declaration: string) =>
		owner.children
			.map((id) => session.graph.state.nodesById.get(id))
			.find((node) => node?.decl_id === declaration);
	const connections = createNativeFormulaConnections();
	const scaleMeasurement = createNativeGraphMeasurement({
		reveal: (id, additive) => panel!.revealGraphNode(String(id), additive),
		find: (query) => panel!.findGraphNodes(query),
		move: (key) => move(key),
		connectionPair: (ids) => ({ source: ids[1], destination: ids[2] }),
		connectionTargetSocket: 'value2',
		startConnection: (id) => startConnection(id, 'result'),
		completeConnection: (id) => completeConnection(id, 'value2'),
		undo: () => history('undo'),
		position(id) {
			const owner = session.graph.state.nodesById.get(id);
			const field = owner ? child(owner, 'position') : null;
			if (field?.data.kind !== 'parameter' || field.data.param.value.kind !== 'vec2')
				throw new Error('Missing exact Formula position');
			return field.data.param.value.value;
		},
		targets: (id) => connections(session.graph.state, session.graph.state.nodesById.get(id)!.uuid),
		uuid: (id) => session.graph.state.nodesById.get(id)!.uuid,
		work: () => panel?.graphInteractionState()?.work,
		pending: () => session.editOperations.size + session.userActionPendingCount
	});
	export const scaleReady = scaleMeasurement.ready;
	export const scaleTopology = scaleMeasurement.topology;
	export const measureScale = scaleMeasurement.run;
	export const load = (path: string) =>
		session.runEditBarrier(() => session.client.projectLoad(path));
	export const status = () => ({
		heldSnapshot,
		latestSnapshot,
		connection: session.status,
		generation: session.snapshotGeneration,
		endpoint: panel?.graphInteractionState()?.endpoint,
		message: panel?.graphInteractionState()?.message,
		pending: session.editOperations.size,
		treeNodes: session.graph.state.nodesById.size
	});
	export const states = () =>
		[...session.graph.state.nodesById.values()]
			.filter((node) => node.node_type === 'alchemist_anode')
			.map((node) => {
				const position = child(node, 'position');
				return {
					id: node.node_id,
					uuid: node.uuid,
					label: node.meta.label,
					type: anodeType(node),
					enabled: node.meta.enabled,
					collapsed: node.meta.presentation?.collapsed === true,
					position: position?.data.kind === 'parameter' ? position.data.param.value : null,
					readOnly: position?.data.kind === 'parameter' && position.data.param.read_only
				};
			})
			.sort((a, b) => a.label.localeCompare(b.label));
	export const transitions = () =>
		[...session.graph.state.nodesById.values()]
			.filter((node) => node.node_type === 'alchemist_connection')
			.map((node) => ({
				id: node.node_id,
				uuid: node.uuid,
				target: child(node, 'target_node')?.data
			}));
	export const reveal = (id: number, additive = false) =>
		panel!.revealGraphNode(String(id), additive);
	export async function move(key: string, large = false) {
		const accepted = panel!.moveGraphSelection(key, large);
		await idle();
		return accepted;
	}
	export async function collapse(id: number) {
		const accepted = panel!.collapseGraphNode(String(id));
		await idle();
		return accepted;
	}
	export async function startConnection(id: number, socket = 'value') {
		const accepted = panel!.connectGraphSocket(String(id), socket, 'output');
		await frames();
		return accepted;
	}
	export async function completeConnection(id: number, socket = 'value1') {
		const accepted = panel!.connectGraphSocket(String(id), socket, 'input');
		await idle();
		return accepted;
	}
	export async function history(kind: 'undo' | 'redo') {
		await session.runEditBarrier(() => session.sendIntent({ kind }));
		await idle();
	}
	export const save = (path: string) =>
		session.runEditBarrier(() => session.client.projectSave(path));
	export const refresh = () => session.refreshSnapshot();
	export function holdSnapshotResponse() {
		if (heldRefresh) throw new Error('A snapshot is already held');
		holdNextSnapshot = true;
		heldRefresh = session.refreshSnapshot();
	}
	export async function releaseSnapshotResponse() {
		if (!releaseSnapshot || !heldRefresh) throw new Error('No held snapshot response');
		releaseSnapshot();
		releaseSnapshot = null;
		const result = await heldRefresh;
		heldRefresh = null;
		return result;
	}
	export const backendLayout = async () =>
		(await session.client.snapshot(wholeGraphScope)).nodes
			.filter((node) => node.decl_id === 'position' && node.data.kind === 'parameter')
			.map((node) => ({ id: node.node_id, data: node.data }));
	export const historyLength = async () =>
		(await session.client.snapshot(wholeGraphScope)).history.undo_len;
	export async function remount() {
		mounted = false;
		await frames();
		mounted = true;
		await frames();
	}
</script>

<div class="fixture">
	{#if mounted}<AlchemistEditorPanel bind:this={panel} {...props} />{/if}
</div>

<style>
	:global(body) {
		margin: 0;
	}
	.fixture {
		inline-size: 70rem;
		block-size: 36rem;
		color: #d8dfeb;
		--gc-color-text: #d8dfeb;
		--gc-color-background: #11151b;
	}
</style>
