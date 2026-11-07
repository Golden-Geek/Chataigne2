<script lang="ts">
	import { onDestroy } from 'svelte';
	import { appState } from '../../../store/workbench.svelte';
	import NumberEditor from '../../../components/panels/inspector/parameters/NumberEditor.svelte';
	import MultiNumberEditor from '../../../components/panels/inspector/parameters/MultiNumberEditor.svelte';
	import DashboardNumber from '../../../components/panels/dashboard/DashboardNodeWidgetNumberSliderContent.svelte';
	import { createFixtureSession, fixtureNode } from './session';
	import type { UiEditIntent, UiNodeDto, ParamValue, UiParamConstraints } from '../../../types';
	const fixture = createFixtureSession();
	appState.session = fixture.session;
	let nodes = $state.raw<UiNodeDto[]>([]);
	let mounted = $state(true);
	let records: UiEditIntent[] = [];
	let sequence = 0;
	let hold = false;
	let pending: (() => void)[] = [];
	const node = (
		id: number,
		label: string,
		value: ParamValue,
		widget: string,
		unit: string,
		constraints: Partial<UiParamConstraints> = {}
	) => {
		const result = fixtureNode(id, label, [], 'parameter');
		result.data = {
			kind: 'parameter',
			param: {
				value,
				default_value: value,
				event_behaviour: 'Append',
				read_only: false,
				constraints: { enum_options: [], policy: 'Reject', ...constraints },
				ui_hints: { widget, unit },
				control: { mode: 'manual', spec: { mode: 'manual' } },
				reference_allowed_targets: [],
				reference_visible_nodes: []
			}
		};
		return result;
	};
	export function setup() {
		records = [];
		mounted = true;
		hold = false;
		pending = [];
		sequence = 0;
		nodes = [
			node(2, 'Milliseconds', { kind: 'float', value: 0.25 }, 'time', 'ms', {
				range: { kind: 'uniform', min: 0, max: 10 },
				step: 0.25
			}),
			node(3, 'Seconds', { kind: 'float', value: 1.25 }, 'time', 's', {
				range: { kind: 'uniform', min: 0, max: 10 },
				step: 0.0001
			}),
			node(4, 'Bounded', { kind: 'float', value: 2 }, 'time_slider', 's', {
				range: { kind: 'uniform', min: 0, max: 10 },
				step: 0.5
			}),
			node(5, 'Unbounded', { kind: 'float', value: 3.5 }, 'time', 'ms', { step: 0.25 }),
			node(6, 'Read only', { kind: 'float', value: 5 }, 'time', 's'),
			node(7, 'Disabled', { kind: 'float', value: 5 }, 'time', 's'),
			node(8, 'Integer', { kind: 'int', value: 0 }, '', '', {
				range: { kind: 'uniform', min: 0, max: 100 },
				step: 1
			}),
			node(9, 'Vector', { kind: 'vec2', value: [0, 0] }, '', '', {
				range: { kind: 'uniform', min: -10, max: 10 },
				step: 0.25
			}),
			node(10, 'Free precision', { kind: 'float', value: 1.234567 }, 'time', 's')
		];
		if (nodes[4].data.kind === 'parameter') nodes[4].data.param.read_only = true;
		nodes[5].meta.enabled = false;
		fixture.loadNodes([
			fixtureNode(0, 'Project', [1]),
			fixtureNode(
				1,
				'Parameters',
				nodes.map((node) => node.node_id)
			),
			...nodes
		]);
	}
	fixture.handleIntent((intent) => {
		records.push(structuredClone(intent));
		if (intent.kind === 'beginEdit') return Promise.resolve(fixture.editAcknowledgement(true));
		if (intent.kind === 'endEdit') return Promise.resolve(fixture.editAcknowledgement(false));
		if (intent.kind !== 'setParam' && intent.kind !== 'setParamComponent')
			throw new Error(`Unexpected ${intent.kind}`);
		const current = fixture.session.graph.state.nodesById.get(intent.node);
		if (current?.data.kind !== 'parameter') throw new Error('Missing Parameter');
		const old = current.data.param.value;
		let value: ParamValue;
		if (intent.kind === 'setParam') value = intent.value;
		else {
			if (old.kind !== 'vec2') throw new Error('Expected vector');
			const next: [number, number] = [...old.value];
			next[intent.component] = intent.value;
			value = { kind: 'vec2', value: next };
		}
		const time = { tick: ++sequence, micro: 0, seq: 0 };
		const publish = () =>
			fixture.session.graph.applyEvent({
				time,
				kind: { kind: 'paramChanged', param: intent.node, old_value: old, new_value: value }
			});
		if (hold) pending.push(publish);
		else publish();
		return Promise.resolve({
			...fixture.editAcknowledgement(),
			earliest_event_time: time,
			latest_event_time: time
		});
	});
	export function inspect() {
		return {
			records,
			active: fixture.session.hasActiveEditSession,
			values: nodes.map((node) => fixture.session.graph.state.paramsById.get(node.node_id)?.value),
			pending: pending.length
		};
	}
	export async function holdEvents() {
		await fixture.connect();
		if (fixture.session.status !== 'connected' || fixture.session.loading.visible)
			throw Error('Held publication requires a connected ready session');
		hold = true;
	}
	export function releaseEvents() {
		hold = false;
		for (const publish of pending.splice(0)) publish();
	}
	export function unmount() {
		mounted = false;
	}
	setup();
	onDestroy(() => {
		fixture.disconnect();
		appState.session = null;
	});
</script>

<main>
	{#if mounted}
		{#each nodes as node (node.uuid)}
			<section data-numeric={node.meta.label}>
				<h2>{node.meta.label}</h2>
				{#if node.node_id === 9}<MultiNumberEditor {node} />{:else}<NumberEditor {node} />{/if}
			</section>
		{/each}
		<section data-numeric="Dashboard">
			<h2>Dashboard</h2>
			<DashboardNumber targetNode={nodes[1]} />
		</section>
	{/if}
</main>

<style>
	main {
		padding: 2rem;
		max-inline-size: 40rem;
	}
	section {
		block-size: 5rem;
	}
	h2 {
		font-size: 1rem;
		margin: 0 0 0.5rem;
	}
</style>
