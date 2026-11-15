<script lang="ts">
	import { onMount } from 'svelte';
	import { appState } from '../../../../../store/workbench.svelte';
	import type { ParamValue, UiNodeDto } from '../../../../../types';
	import type { PanelApi, PanelParams } from '../../../../../dockview/panel-types';
	import { PERSISTED_PANEL_STATE_KEY } from '../../../../../dockview/panel-persistence';
	import { createFixtureSession, fixtureNode } from '../../../../../tests/browser/fixture/session';
	import InspectorPanel from '../../InspectorPanel.svelte';
	import { auditCurveBridge } from '../curve-bridge.svelte';
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
	{#each ['Curve A', 'Curve A key 0', 'Curve A key 1', 'Curve B key 0'] as label}<button
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
</main>

<style>
	:global(:root) {
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
