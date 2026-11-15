<script lang="ts">
	import { onMount } from 'svelte';
	import { appState } from '../../../../../store/workbench.svelte';
	import type { ParamValue, UiNodeDto } from '../../../../../types';
	import type { PanelApi, PanelParams } from '../../../../../dockview/panel-types';
	import { PERSISTED_PANEL_STATE_KEY } from '../../../../../dockview/panel-persistence';
	import { createFixtureSession, fixtureNode } from '../../../../../tests/browser/fixture/session';
	import InspectorPanel from '../../InspectorPanel.svelte';
	import { auditCurveBridge } from '../curve-bridge.svelte';
	import { executeCommand } from '../../../../../store/commands.svelte';
	const fixture = createFixtureSession();
	const nodes: UiNodeDto[] = [fixtureNode(0, 'Project', [1, 20, 40]), fixtureNode(40, 'Unrelated')];
	const references = new Map<string, UiNodeDto>();
	function param(id: number, label: string, declaration: string, value: ParamValue) {
		const node = fixtureNode(id, label, [], 'parameter');
		node.decl_id = declaration;
		node.data = {
			kind: 'parameter',
			param: {
				value,
				default_value: value,
				event_behaviour: 'Coalesce',
				read_only: false,
				constraints: { enum_options: [], policy: 'ClampAdapt' },
				ui_hints: {},
				control: { mode: 'manual', spec: { mode: 'manual' } },
				reference_allowed_targets: [],
				reference_visible_nodes: []
			}
		};
		return node;
	}
	for (const [base, label] of [
		[1, 'Curve A'],
		[20, 'Curve B']
	] as const) {
		const curve = fixtureNode(base, label, [base + 1, base + 7], 'animation_curve');
		curve.creatable_user_items = [
			{
				node_type: 'animation_curve_key',
				item_kind: 'animation_curve_key',
				label: 'Key',
				menu_path: [],
				initial_params: [],
				select_when_created: true
			}
		];
		nodes.push(curve);
		references.set(label, curve);
		for (const [offset, value] of [
			[1, 0],
			[7, 1]
		]) {
			const id = base + offset,
				handleBase = 100 + base * 10 + offset * 5,
				key = fixtureNode(
					id,
					`${label} key ${value}`,
					[id + 1, id + 2, id + 3],
					'animation_curve_key'
				);
			nodes.push(
				key,
				param(id + 1, 'Position', 'position', { kind: 'float', value }),
				param(id + 2, 'Value', 'value', { kind: 'float', value }),
				fixtureNode(
					id + 3,
					'Easing',
					[id + 4, handleBase, handleBase + 1, handleBase + 2, handleBase + 3],
					'animation_curve_easing'
				),
				param(id + 4, 'Kind', 'kind', { kind: 'enum', value: 'bezier' }),
				param(handleBase, 'Out Position', 'out_position', { kind: 'float', value: 1 / 3 }),
				param(handleBase + 1, 'Out Value', 'out_value', { kind: 'float', value: 1 / 3 }),
				param(handleBase + 2, 'In Position', 'in_position', { kind: 'float', value: -1 / 3 }),
				param(handleBase + 3, 'In Value', 'in_value', { kind: 'float', value: -1 / 3 })
			);
			nodes.find((node) => node.node_id === id + 3)!.decl_id = 'easing';
			references.set(key.meta.label, key);
		}
	}
	fixture.loadNodes(nodes);
	let inspector = $state<ReturnType<typeof InspectorPanel>>();
	let edits = $state(0),
		writes = $state(0),
		comparison = $state(false);
	let guardResult = $state('pending');
	let removals = $state(0);
	let sequence = 0;
	let holdBegin = false;
	let pendingBegins = $state(0);
	const beginReleases: Array<() => void> = [];
	let fitOwners = $state<string[]>([]);
	let createOwners = $state<string[]>([]),
		pendingCreates = $state(0),
		lastCreated = $state(0);
	let holdCreate = false,
		nextKeyId = 1000;
	const createReleases: Array<() => void> = [];
	let editorsVisible = $state(true),
		commandResult = $state('idle');
	let createdPosition = $derived.by(() => {
		const value = fixture.session.graph.state.paramsById.get(lastCreated + 1)?.value;
		return value?.kind === 'float' ? value.value : null;
	});
	async function runCurveCommand(command: 'edit.copy' | 'edit.paste') {
		const canvas = document.querySelector('[aria-label="Primary curve Inspector"] canvas');
		if (!(canvas instanceof HTMLCanvasElement)) throw Error('Missing Curve canvas');
		canvas.focus();
		commandResult = 'pending';
		commandResult = String(await executeCommand(command, { source: 'menu' }));
	}
	function insertKey(parent: number, decoy: boolean) {
		const id = nextKeyId;
		nextKeyId += 16;
		const key = fixtureNode(
			id,
			`${decoy ? 'Decoy' : 'Created'} ${id}`,
			[id + 1, id + 2, id + 3],
			'animation_curve_key'
		);
		const easing = fixtureNode(
			id + 3,
			'Easing',
			[id + 4, id + 5, id + 6, id + 7, id + 8],
			'animation_curve_easing'
		);
		easing.decl_id = 'easing';
		const subtree = [
			key,
			param(id + 1, 'Position', 'position', { kind: 'float', value: 0 }),
			param(id + 2, 'Value', 'value', { kind: 'float', value: 0 }),
			easing,
			param(id + 4, 'Kind', 'kind', { kind: 'enum', value: 'linear' }),
			...['out_position', 'out_value', 'in_position', 'in_value'].map((decl, index) =>
				param(id + 5 + index, decl, decl, { kind: 'float', value: index < 2 ? 1 / 3 : -1 / 3 })
			)
		];
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: {
				kind: 'graphTransaction',
				tx_id: sequence,
				epoch: 1,
				base_graph_version: sequence - 1,
				next_graph_version: sequence,
				ops: [
					{
						kind: 'subtreeInserted',
						parent,
						root: id,
						nodes: subtree,
						parent_children_after: [
							...fixture.session.graph.state.nodesById.get(parent)!.children,
							id
						]
					}
				]
			}
		});
		return key;
	}
	type AccessMode = 'manual' | 'readonly' | 'disabled' | 'controlled' | 'mixed' | 'protected';
	let accessMode = $state<AccessMode>('manual');
	let accessRevision = $state(0);
	async function changeAccess(mode: AccessMode) {
		const copy = structuredClone(nodes),
			byId = new Map(copy.map((node) => [node.node_id, node]));
		const owned = new Set<number>();
		const visit = (id: number) => {
			if (owned.has(id)) return;
			owned.add(id);
			for (const child of byId.get(id)?.children ?? []) visit(child);
		};
		visit(1);
		for (const node of copy) {
			if (!owned.has(node.node_id)) continue;
			if (mode === 'protected' && node.node_id === 2)
				node.meta.user_permissions.can_remove_and_duplicate = false;
			if (node.data.kind !== 'parameter') continue;
			if (mode === 'readonly' || (mode === 'mixed' && node.node_id === 10))
				node.data.param.read_only = true;
			if (mode === 'disabled') node.meta.enabled = false;
			if (mode === 'controlled')
				node.data.param.control = { mode: 'expression', spec: { mode: 'expression' } };
		}
		fixture.loadNodes(copy);
		await fixture.session.refreshSnapshot();
		select('Curve A');
		accessMode = mode;
		accessRevision++;
	}
	function revokeAccess(controlled: boolean) {
		const node = fixture.session.graph.state.nodesById.get(4);
		if (node?.data.kind !== 'parameter') throw Error('Missing Curve value');
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: controlled
				? {
						kind: 'paramControlChanged',
						param: 4,
						old_state: node.data.param.control,
						new_state: { mode: 'expression', spec: { mode: 'expression' } }
					}
				: { kind: 'metaChanged', node: 4, patch: { enabled: false } }
		});
	}
	const api: PanelApi = {
		observeVisibility: (listener) => {
			listener(true);
			return () => {};
		},
		setContentBinding() {},
		setTitle() {},
		close() {},
		updateParams() {},
		getParams: <T extends PanelParams>() => ({}) as T
	};
	function locked(label: string) {
		return {
			[PERSISTED_PANEL_STATE_KEY]: {
				inspectorLocked: true,
				inspectorLockedNodeUuid: fixture.session.graph.state.nodesById.get(
					references.get(label)!.node_id
				)!.uuid,
				inspectorLockedDocument: fixture.session.graph.state.nodesById.get(0)!.uuid
			}
		};
	}
	function select(label: string) {
		fixture.session.selectNode(references.get(label)!.node_id, 'REPLACE');
	}
	fixture.handleIntent(async (intent) => {
		edits++;
		const generation = fixture.session.snapshotGeneration;
		if (intent.kind === 'beginEdit' && holdBegin) {
			pendingBegins++;
			await new Promise<void>((resolve) => beginReleases.push(resolve));
			pendingBegins--;
			if (generation !== fixture.session.snapshotGeneration)
				return { success: true, status: 'applied' };
		}
		if (intent.kind === 'fitAnimationCurvePath')
			fitOwners = [
				...fitOwners,
				fixture.session.graph.state.nodesById.get(intent.curve)?.meta.label ??
					`missing:${intent.curve}`
			];
		if (intent.kind === 'createUserItem') {
			createOwners = [
				...createOwners,
				fixture.session.graph.state.nodesById.get(intent.parent)!.meta.label
			];
			if (holdCreate) insertKey(intent.parent, true);
			const key = insertKey(intent.parent, false);
			lastCreated = key.node_id;
			if (holdCreate) {
				pendingCreates++;
				await new Promise<void>((resolve) => createReleases.push(resolve));
				pendingCreates--;
			}
			return {
				...fixture.editAcknowledgement(),
				created_node: { node_id: key.node_id, uuid: key.uuid }
			};
		}
		if (intent.kind === 'removeNode' || intent.kind === 'removeNodes') removals++;
		if (intent.kind === 'setParam') {
			writes++;
			const current = fixture.session.graph.state.nodesById.get(intent.node);
			if (current?.data.kind !== 'parameter') throw Error('Missing Curve Parameter');
			fixture.session.graph.applyEvent({
				time: { tick: ++sequence, micro: 0, seq: 0 },
				kind: {
					kind: 'paramChanged',
					param: intent.node,
					old_value: current.data.param.value,
					new_value: intent.value
				}
			});
		}
		return fixture.editAcknowledgement(
			intent.kind === 'beginEdit' ? true : intent.kind === 'endEdit' ? false : undefined
		);
	});
	onMount(() => {
		const previous = appState.session;
		appState.session = fixture.session;
		void fixture.connect().then(() => select('Curve A'));
		return () => {
			fixture.disconnect();
			appState.session = previous;
		};
	});
</script>

<div class="toolbar">
	<button
		onclick={() => {
			guardResult = auditCurveBridge();
		}}>Run curve guard checks</button>
	<output aria-label="Curve guard checks">{guardResult}</output>
	<output aria-label="Curve removal intents">{removals}</output>
	<output aria-label="Curve pending begins">{pendingBegins}</output>
	<output aria-label="Curve fit owners">{fitOwners.join('|')}</output>
	<output aria-label="Curve create owners">{createOwners.join('|')}</output>
	<output aria-label="Curve pending creates">{pendingCreates}</output>
	<output aria-label="Curve last created key">{lastCreated}</output>
	<output aria-label="Curve created position">{createdPosition}</output>
	<output aria-label="Curve command result">{commandResult}</output>
	<button onclick={() => runCurveCommand('edit.copy')}>Copy Curve keys</button>
	<button onclick={() => runCurveCommand('edit.paste')}>Paste Curve keys</button>
	<button
		onclick={() => {
			editorsVisible = false;
		}}>Hide Curve editors</button>
	<button
		onclick={() => {
			holdCreate = true;
		}}>Hold Curve Create</button>
	<button
		onclick={() => {
			holdCreate = false;
			for (const release of createReleases.splice(0)) release();
		}}>Release Curve Create</button>
	<button
		onclick={() => {
			holdBegin = true;
		}}>Hold Curve Begin</button>
	<button
		onclick={() => {
			holdBegin = false;
			for (const release of beginReleases.splice(0)) release();
		}}>Release Curve Begin</button>
	<output aria-label="Curve access mode">{accessMode}</output>
	<output aria-label="Curve access revision">{accessRevision}</output>
	{#each ['manual', 'readonly', 'disabled', 'controlled', 'mixed', 'protected'] as mode}
		<button onclick={() => changeAccess(mode as AccessMode)}>Curve access {mode}</button>
	{/each}
	<button onclick={() => revokeAccess(false)}>Disable active Curve value</button>
	<button onclick={() => revokeAccess(true)}>Control active Curve value</button>
	<button
		onclick={async () => {
			fixture.loadNodes(structuredClone(nodes));
			await fixture.session.refreshSnapshot();
			select('Curve A');
		}}>Replace Curve snapshot</button>
	<output aria-label="Selected curve nodes"
		>{fixture.session.selectedNodesIds
			.map((id) => fixture.session.graph.state.nodesById.get(id)?.meta.label)
			.join('|')}</output>
	<output aria-label="Curve edits">{edits}</output><output aria-label="Curve writes"
		>{writes}</output>
	{#each ['Curve A', 'Curve A key 0', 'Curve A key 1', 'Curve B', 'Curve B key 0'] as label}<button
			onclick={() => select(label)}>Select {label}</button
		>{/each}
	<button onclick={() => fixture.session.selectNodes([2, 8], 'REPLACE')}>Select both keys</button>
	<button onclick={() => fixture.session.selectNode(4, 'REPLACE')}>Select key value</button>
	<button
		onclick={() => {
			inspector?.setPanelState({
				panelId: 'curve-main',
				panelType: 'Inspector',
				title: 'Inspector',
				params: locked('Curve A')
			});
			comparison = true;
		}}>Open locked editors</button>
</div>
<main>
	{#if editorsVisible}
		<section aria-label="Primary curve Inspector">
			<InspectorPanel
				bind:this={inspector}
				panelApi={api}
				panelId="curve-main"
				panelType="Inspector"
				title="Inspector"
				params={{}} />
		</section>
		{#if comparison}<section aria-label="Other curve Inspector">
				<InspectorPanel
					panelApi={api}
					panelId="curve-other"
					panelType="Inspector"
					title="Inspector"
					params={locked('Curve B')} />
			</section>{/if}
	{/if}
</main>

<style>
	:global(:root) {
		--gc-color-curve-key-selected: #f100f1;
		--gc-color-curve-handle-out: #fd00fd;
		--gc-color-curve-handle-in: #00fdfd;
	}
	:global(body) {
		margin: 0;
		background: #202020;
		color: #eee;
		font-family: sans-serif;
	}
	.toolbar {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}
	output {
		white-space: nowrap;
		overflow: hidden;
	}
	main {
		display: flex;
		height: 80vh;
	}
	section {
		width: 50%;
		min-width: 0;
		min-height: 0;
		overflow: auto;
	}
</style>
