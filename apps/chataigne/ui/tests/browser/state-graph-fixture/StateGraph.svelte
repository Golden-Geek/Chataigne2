<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { appState, createWorkbenchSession } from 'golden_ui/store/workbench.svelte';
	import type { PanelApi, PanelParams, PanelProps, UiNodeDto } from 'golden_ui';
	import { wholeGraphScope } from 'golden_ui/types';
	import StateMachinePanel from '../../../src/lib/systems/state_machine/components/StateMachinePanel.svelte';
	import { createNativeGraphMeasurement } from '../app-graph-scale/measurement';
	import {
		STATE_GRAPH_INPUT,
		STATE_GRAPH_OUTPUT
	} from '../../../src/lib/systems/state_machine/generated/state-graph-constants';
	const base = new URLSearchParams(location.search).get('backend');
	if (!base) throw new Error('Missing owned native backend URL');
	const session = createWorkbenchSession({
		httpBaseUrl: `${base}/api/ui`,
		wsUrl: `${base.replace('http:', 'ws:')}/api/ui/ws`,
		enableGlobalShortcuts: false
	});
	appState.session = session;
	onMount(() => session.mount());
	let panel = $state<ReturnType<typeof StateMachinePanel>>();
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
		panelId: 'native-states',
		panelType: 'StateMachine',
		title: 'Native states',
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
	const scaleMeasurement = createNativeGraphMeasurement({
		reveal: (id, additive) => panel!.revealGraphNode(String(id), additive),
		find: (query) => panel!.findGraphNodes(query),
		move: (key) => move(key),
		startConnection: (id) => startConnection(id),
		completeConnection: (id) => completeConnection(id),
		undo: () => history('undo'),
		position(id) {
			const owner = session.graph.state.nodesById.get(id);
			const field = owner ? child(owner, 'position') : null;
			if (field?.data.kind !== 'parameter' || field.data.param.value.kind !== 'vec2')
				throw new Error('Missing exact State position');
			return field.data.param.value.value;
		},
		targets(id) {
			const owner = session.graph.state.nodesById.get(id);
			const transitions = owner ? child(owner, 'transitions') : null;
			return (transitions?.children ?? []).flatMap((id) => {
				const node = session.graph.state.nodesById.get(id);
				if (node?.node_type !== 'state_transition') return [];
				const field = child(node, 'target');
				const target =
					field?.data.kind === 'parameter' && field.data.param.value.kind === 'reference'
						? field.data.param.value.uuid
						: null;
				return [{ id, uuid: node.uuid, target }];
			});
		},
		uuid: (id) => session.graph.state.nodesById.get(id)!.uuid,
		work: () => panel?.graphInteractionState()?.work,
		pending: () => session.editOperations.size + session.userActionPendingCount
	});
	export const scaleReady = scaleMeasurement.ready;
	export const scaleTopology = scaleMeasurement.topology;
	export const measureScale = scaleMeasurement.run;
	export const status = () => ({
		connection: session.status,
		generation: session.snapshotGeneration,
		endpoint: panel?.graphInteractionState()?.endpoint,
		message: panel?.graphInteractionState()?.message,
		pending: session.editOperations.size,
		treeNodes: session.graph.state.nodesById.size
	});
	export const states = () =>
		[...session.graph.state.nodesById.values()]
			.filter((node) => node.node_type === 'state')
			.map((node) => {
				const position = child(node, 'position');
				return {
					id: node.node_id,
					uuid: node.uuid,
					label: node.meta.label,
					enabled: node.meta.enabled,
					collapsed: node.meta.presentation?.collapsed === true,
					position: position?.data.kind === 'parameter' ? position.data.param.value : null,
					readOnly: position?.data.kind === 'parameter' && position.data.param.read_only
				};
			})
			.sort((a, b) => a.label.localeCompare(b.label));
	export const transitions = () =>
		[...session.graph.state.nodesById.values()]
			.filter((node) => node.node_type === 'state_transition')
			.map((node) => ({ id: node.node_id, uuid: node.uuid, target: child(node, 'target')?.data }));
	export const reveal = (id: number, additive = false) =>
		panel!.revealGraphNode(String(id), additive);
	export const find = (query: string) => panel!.findGraphNodes(query);
	export const load = (path: string) =>
		session.runEditBarrier(() => session.client.projectLoad(path));
	export const settle = idle;
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
	export async function startConnection(id: number) {
		const accepted = panel!.connectGraphSocket(String(id), STATE_GRAPH_OUTPUT, 'output');
		await frames();
		return accepted;
	}
	export async function completeConnection(id: number) {
		const accepted = panel!.connectGraphSocket(String(id), STATE_GRAPH_INPUT, 'input');
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
	{#if mounted}<StateMachinePanel bind:this={panel} {...props} />{/if}
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
