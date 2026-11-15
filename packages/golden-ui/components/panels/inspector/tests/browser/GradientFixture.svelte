<script lang="ts">
	import { onMount } from 'svelte';
	import { appState } from '../../../../../store/workbench.svelte';
	import type { ParamValue, UiNodeDto } from '../../../../../types';
	import type { PanelApi, PanelParams } from '../../../../../dockview/panel-types';
	import { PERSISTED_PANEL_STATE_KEY } from '../../../../../dockview/panel-persistence';
	import { createFixtureSession, fixtureNode } from '../../../../../tests/browser/fixture/session';
	import InspectorPanel from '../../InspectorPanel.svelte';
	const fixture = createFixtureSession();
	const nodes: UiNodeDto[] = [fixtureNode(0, 'Project', [1, 20, 40]), fixtureNode(40, 'Other')];
	for (const [base, label, count] of [
		[1, 'Gradient A', 3],
		[20, 'Gradient B', 2]
	] as const) {
		const owner = fixtureNode(base, label, [], 'gradient');
		nodes.push(owner);
		for (let i = 0; i < count; i++) {
			const id = base + 1 + i * 5;
			owner.children.push(id);
			nodes.push(fixtureNode(id, `${label} stop ${i}`, [id + 1, id + 2, id + 3], 'gradient_stop'));
			const values: [string, string, ParamValue][] = [
				['Position', 'position', { kind: 'float', value: (i + 1) / (count + 1) }],
				['Color', 'color', { kind: 'color', value: [i / count, 0.3, 1, 1] }],
				['Interpolation', 'interpolation', { kind: 'enum', value: 'linear' }]
			];
			values.forEach(([name, declaration, value], field) => {
				const parameter = fixtureNode(id + field + 1, name, [], 'parameter');
				parameter.decl_id = declaration;
				parameter.data = {
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
				nodes.push(parameter);
			});
		}
	}
	fixture.loadNodes(nodes);
	let writes = $state(0),
		removals = $state(0),
		begins = $state(0),
		ends = $state(0);
	let visible = $state(true),
		comparison = $state(false),
		sequence = 100;
	let inspector = $state<ReturnType<typeof InspectorPanel>>();
	let holdBegin = false;
	const beginReleases: (() => void)[] = [];
	fixture.handleIntent(async (intent) => {
		if (intent.kind === 'beginEdit') {
			begins++;
			if (holdBegin) await new Promise<void>((resolve) => beginReleases.push(resolve));
		}
		if (intent.kind === 'endEdit') ends++;
		if (intent.kind === 'removeNode') removals++;
		if (intent.kind === 'setParam') {
			writes++;
			const current = fixture.session.graph.state.nodesById.get(intent.node);
			if (current?.data.kind !== 'parameter') throw Error('Missing Gradient Parameter');
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
	const locked = (id: number) => ({
		[PERSISTED_PANEL_STATE_KEY]: {
			inspectorLocked: true,
			inspectorLockedNodeUuid: fixture.session.graph.state.nodesById.get(id)!.uuid,
			inspectorLockedDocument: fixture.session.graph.state.nodesById.get(0)!.uuid
		}
	});
	const select = (id: number) => fixture.session.selectNode(id, 'REPLACE');
	const disablePosition = () =>
		fixture.session.graph.applyEvent({
			time: { tick: ++sequence, micro: 0, seq: 0 },
			kind: { kind: 'metaChanged', node: 3, patch: { enabled: false } }
		});
	onMount(() => {
		const previous = appState.session;
		appState.session = fixture.session;
		void fixture.connect().then(() => select(1));
		return () => {
			fixture.disconnect();
			appState.session = previous;
		};
	});
</script>

<nav aria-label="Gradient fixture controls">
	<button
		onclick={() => {
			holdBegin = true;
		}}>Hold Gradient begin</button>
	<button
		onclick={() => {
			holdBegin = false;
			for (const release of beginReleases.splice(0)) release();
		}}>Release Gradient begin</button>
	<button onclick={() => select(1)}>Select Gradient A</button>
	<button onclick={() => select(2)}>Select Gradient stop A</button>
	<button onclick={() => select(3)}>Select Gradient position</button>
	<button onclick={() => select(21)}>Select Gradient stop B</button>
	<button onclick={() => select(40)}>Select other Gradient object</button>
	<button onclick={disablePosition}>Disable Gradient position</button>
	<button
		onclick={async () => {
			fixture.loadNodes(nodes);
			await fixture.session.refreshSnapshot();
			select(1);
		}}>Replace Gradient document</button>
	<button
		onclick={() => {
			inspector?.setPanelState({
				panelId: 'gradient-a',
				panelType: 'Inspector',
				title: 'Inspector',
				params: locked(1)
			});
			comparison = true;
		}}>Open locked Gradient editors</button>
	<button
		onclick={() => {
			visible = !visible;
		}}>{visible ? 'Hide Gradient editors' : 'Show Gradient editors'}</button>
</nav>
<output aria-label="Selected Gradient nodes"
	>{fixture.session.selectedNodesIds
		.map((id) => fixture.session.graph.state.nodesById.get(id)?.meta.label)
		.join('|')}</output>
<output aria-label="Gradient writes">{writes}</output>
<output aria-label="Gradient removals">{removals}</output>
<output aria-label="Gradient begins">{begins}</output>
<output aria-label="Gradient ends">{ends}</output>
<output aria-label="Gradient generation">{fixture.session.snapshotGeneration}</output>
<main>
	{#if visible}
		<section aria-label="Primary Gradient Inspector">
			<InspectorPanel
				bind:this={inspector}
				panelApi={api}
				panelId="gradient-a"
				panelType="Inspector"
				title="Inspector"
				params={{}} />
		</section>
		{#if comparison}<section aria-label="Other Gradient Inspector">
				<InspectorPanel
					panelApi={api}
					panelId="gradient-b"
					panelType="Inspector"
					title="Inspector"
					params={locked(20)} />
			</section>{/if}
	{/if}
</main>

<style>
	main {
		display: flex;
		gap: 1rem;
	}
	section {
		inline-size: 36rem;
		block-size: 36rem;
	}
</style>
