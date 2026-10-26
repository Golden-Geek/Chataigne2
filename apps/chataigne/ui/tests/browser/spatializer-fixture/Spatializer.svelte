<script lang="ts">
	import { onMount, tick } from 'svelte';
	import type { PanelApi, PanelParams, PanelProps, ParamValue, UiNodeDto } from 'golden_ui';
	import { appState, createWorkbenchSession } from 'golden_ui/store/workbench.svelte';
	import { executeCommand } from 'golden_ui/store/commands.svelte';
	import { writePanelPersistedState } from 'golden_ui/dockview/panel-persistence';
	import SpatializerEditorPanel from '../../../src/lib/panels/modules/SpatializerEditorPanel.svelte';
	import { createDefaultUiClient } from 'golden_ui';
	import {
		SPATIALIZER_NAMESPACE,
		SPATIALIZER_EDIT_OPERATION
	} from '../../../src/lib/panels/modules/spatializer/generated/constants';
	let holdResync = false,
		heldResync = $state(false),
		releaseResync: (() => void) | null = null;
	const base = new URLSearchParams(location.search).get('backend');
	if (!base) throw new Error('Missing owned backend URL');
	const session = createWorkbenchSession({
		httpBaseUrl: `${base}/api/ui`,
		wsUrl: `${base.replace('http:', 'ws:')}/api/ui/ws`,
		enableGlobalShortcuts: false,
		transportFactory: (options = {}) =>
			createDefaultUiClient({
				...options,
				async onResyncRequired(scope, plane, reason) {
					if (holdResync) {
						heldResync = true;
						await new Promise<void>((resolve) => {
							releaseResync = resolve;
						});
						heldResync = false;
					}
					if (!options.onResyncRequired) throw new Error('Missing fixture resync callback');
					return options.onResyncRequired(scope, plane, reason);
				}
			})
	});
	appState.session = session;
	onMount(() => session.mount());
	let mounted = $state(true);
	let panel = $state<ReturnType<typeof SpatializerEditorPanel>>();
	let moduleId: number | null = null;
	let params = $state.raw<PanelParams>({});
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
		panelId: 'native-spatializer',
		panelType: 'SpatializerEditor',
		title: 'Spatializer',
		params
	} satisfies PanelProps);
	const nodes = () => [...session.graph.state.nodesById.values()];
	const child = (owner: UiNodeDto, declaration: string) =>
		owner.children
			.map((id) => session.graph.state.nodesById.get(id))
			.find((node) => node?.decl_id === declaration || node?.decl_id.endsWith(`/${declaration}`));
	const frames = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
	};
	const waitFor = async <T,>(read: () => T | null | undefined): Promise<T> => {
		const deadline = performance.now() + 10000;
		for (;;) {
			const value = read();
			if (value) return value;
			if (performance.now() > deadline)
				throw new Error('Spatializer fixture publication timed out');
			await frames();
		}
	};
	const create = async (parent: UiNodeDto, node_type: string, label: string) => {
		await session.sendIntent({ kind: 'createUserItem', parent: parent.node_id, node_type, label });
		return waitFor(() =>
			nodes().find(
				(node) =>
					session.graph.state.nodesById.get(parent.node_id)?.children.includes(node.node_id) &&
					node.node_type === node_type &&
					node.meta.label === label
			)
		);
	};
	export const status = () => ({
		heldResync,
		pending: session.editOperations.size,
		connection: session.status,
		generation: session.snapshotGeneration,
		nodes: nodes().length
	});
	export const endpoints = () =>
		nodes()
			.filter((node) => ['spatializer_source', 'spatializer_target'].includes(node.node_type))
			.map((node) => ({
				id: node.node_id,
				uuid: node.uuid,
				label: node.meta.label,
				position: (child(node, 'position_2d') ?? child(node, 'position_3d'))?.data
			}));
	/** Test arrangement uses generic authored edits; it does not qualify the panel's mutation API. */
	async function setValue(id: number, declaration: string, value: ParamValue) {
		const endpoint = session.graph.state.nodesById.get(id);
		if (!endpoint) throw new Error('Missing fixture endpoint');
		const parameter = await waitFor(() => {
			const latest = session.graph.state.nodesById.get(id);
			return latest ? child(latest, declaration) : null;
		});
		if (parameter.data.kind !== 'parameter') throw new Error('Missing position parameter');
		await session.sendIntent({
			kind: 'setParam',
			node: parameter.node_id,
			value,
			behaviour: parameter.data.param.event_behaviour
		});
		await frames();
	}
	export const position = (id: number, x: number, y: number) =>
		setValue(id, 'position_2d', { kind: 'vec2', value: [x, y] });
	export const position3d = (id: number, x: number, height: number, y: number) =>
		setValue(id, 'position_3d', { kind: 'vec3', value: [x, height, y] });
	export async function configure(key: 'mode' | 'dimensions' | 'value_layout', value: string) {
		const module = moduleId !== null ? session.graph.state.nodesById.get(moduleId) : null;
		const parameters = module ? child(module, 'parameters') : null;
		if (!parameters) throw new Error('Missing Spatializer configuration');
		await setValue(parameters.node_id, key, { kind: 'enum', value });
	}
	export async function setup() {
		const manager = await waitFor(() =>
			nodes().find((node) =>
				node.creatable_user_items.some((item) => item.node_type === 'spatializer_module')
			)
		);
		const module = await create(manager, 'spatializer_module', 'Spatializer');
		moduleId = module.node_id;
		const sources = await waitFor(() =>
			nodes().find((node) => node.node_type === 'spatializer_source_list')
		);
		const targets = await waitFor(() =>
			nodes().find((node) => node.node_type === 'spatializer_target_list')
		);
		const firstEndpoint = (list: UiNodeDto, type: string) =>
			session.graph.state.nodesById
				.get(list.node_id)
				?.children.map((id) => session.graph.state.nodesById.get(id))
				.find((node) => node?.node_type === type);
		const source = await waitFor(() => firstEndpoint(sources, 'spatializer_source'));
		const west = await waitFor(() => firstEndpoint(targets, 'spatializer_target'));
		await session.sendIntent({
			kind: 'patchMeta',
			node: source.node_id,
			patch: { label: 'Probe' }
		});
		await session.sendIntent({ kind: 'patchMeta', node: west.node_id, patch: { label: 'West' } });
		const east = await create(targets, 'spatializer_target', 'East');
		await position(west.node_id, -1, 0);
		await position(east.node_id, 1, 0);
		params = { ...params, moduleNodeId: module.node_id };
		await frames();
		return {
			module: module.node_id,
			source: source.node_id,
			west: west.node_id,
			east: east.node_id
		};
	}
	export async function select(id: number) {
		session.selectNode(id, 'REPLACE');
		await frames();
	}
	export async function debug(enabled: boolean) {
		writePanelPersistedState(panelApi, { debugView: enabled });
		await frames();
	}
	export const refresh = () => session.refreshSnapshot();
	export const home = () => executeCommand('view.home', { source: 'api' });
	export const key = (
		id: number,
		key: string,
		shift = false,
		kind: 'position' | 'radius' | 'freezeRadius' = 'position'
	) => panel!.editEndpointKey(id, key, shift, kind);
	export const begin = (
		id: number,
		kind: 'position' | 'radius' | 'freezeRadius',
		point: { x: number; y: number }
	) => panel!.beginEndpointGesture(id, kind, point);
	export const update = (point: { x: number; y: number }) => panel!.updateEndpointGesture(point);
	export const finish = (cancelled = false) => panel!.finishEndpointGesture(cancelled);
	export const move = (moves: { id: number; position: { x: number; y: number } }[]) =>
		panel!.moveEndpoints(moves);
	export const createAt = (point: { x: number; y: number }) => panel!.createTargetAtWorld(point);
	export const createInView = () => panel!.createTargetInView();
	export const fieldValue = (id: number, key: string) => {
		const owner = session.graph.state.nodesById.get(id),
			node = owner ? child(owner, key) : null;
		return node?.data.kind === 'parameter' ? node.data.param.value : null;
	};
	export const settle = frames;
	export const staleSpace = (id: number) => {
		const module = moduleId !== null ? session.graph.state.nodesById.get(moduleId) : null;
		const endpoint = session.graph.state.nodesById.get(id);
		if (!module || !endpoint) throw new Error('Missing fixture identity');
		return session.invokeAppOperation({
			kind: 'invokeAppOperation',
			namespace: SPATIALIZER_NAMESPACE,
			operation: SPATIALIZER_EDIT_OPERATION,
			payload: {
				module: { node: module.node_id, uuid: module.uuid },
				space: 'xy',
				edit: {
					kind: 'nudge',
					endpoint: { node: endpoint.node_id, uuid: endpoint.uuid },
					delta: [100, 0]
				}
			}
		});
	};
	export const holdReplacement = () => {
		holdResync = true;
	};
	export const resumeReplacement = () => {
		holdResync = false;
		releaseResync?.();
		releaseResync = null;
	};
	export const save = (path: string) =>
		session.runEditBarrier(() => session.client.projectSave(path));
	export const load = (path: string) =>
		session.runEditBarrier(() => session.client.projectLoad(path));
	export const historyLength = async () =>
		(await session.client.snapshot({ kind: 'wholeGraph' })).history.undo_len;
	export const history = (kind: 'undo' | 'redo') =>
		session.runEditBarrier(() => session.sendIntent({ kind }));
	export const fieldWritable = (id: number) => {
		const owner = session.graph.state.nodesById.get(id);
		const node = owner ? (child(owner, 'position_2d') ?? child(owner, 'position_3d')) : null;
		return node?.data.kind === 'parameter' && !node.data.param.read_only;
	};
	export async function remount() {
		mounted = false;
		await frames();
		mounted = true;
		await frames();
	}
</script>

<div class="fixture">
	{#if mounted}<SpatializerEditorPanel bind:this={panel} {...props} />{/if}
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
