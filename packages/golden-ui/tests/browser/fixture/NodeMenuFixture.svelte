<script lang="ts">
	import { onDestroy } from 'svelte';
	import NodeContextMenu from '../../../components/common/NodeContextMenu.svelte';
	import { closeNodeContextMenu } from '../../../store/node-context-menu.svelte';
	import { createFixtureSession, fixtureNode } from './session';
	import type { ParamValue, UiEditIntent, UiNodeDto } from '../../../types';
	let { fixture }: { fixture: ReturnType<typeof createFixtureSession> } = $props();
	let requests: UiEditIntent[] = [];
	let sequence = 0;
	const parameter = (id: number, label: string, value: ParamValue): UiNodeDto => {
		const node = fixtureNode(id, label, [], 'parameter');
		node.meta.user_permissions.can_edit_constraints = true;
		node.data = {
			kind: 'parameter',
			param: {
				value,
				default_value: value,
				event_behaviour: 'Append',
				read_only: false,
				constraints: { enum_options: [], policy: 'ClampAdapt' },
				ui_hints: {},
				control: { mode: 'manual', spec: { mode: 'manual' } },
				reference_allowed_targets: [],
				reference_visible_nodes: []
			}
		};
		return node;
	};
	export function setup() {
		requests = [];
		fixture.loadNodes([
			fixtureNode(0, 'Project', [1]),
			fixtureNode(1, 'Manager', [2, 3]),
			parameter(2, 'Numeric', { kind: 'float', value: 0 }),
			parameter(3, 'Choice', { kind: 'enum', value: 'first' })
		]);
		fixture.handleIntent((intent) => {
			if (intent.kind !== 'setParamConstraints') return null;
			requests.push(structuredClone(intent));
			const node = fixture.session.graph.state.nodesById.get(intent.node)!;
			if (node.data.kind !== 'parameter') throw new Error('Expected parameter');
			fixture.session.graph.applyEvent({
				time: { tick: ++sequence, micro: 0, seq: 0 },
				kind: {
					kind: 'paramConstraintsChanged',
					param: intent.node,
					old_constraints: node.data.param.constraints,
					new_constraints: intent.constraints
				}
			});
			return Promise.resolve(fixture.editAcknowledgement());
		});
	}
	export const intents = () => requests;
	onDestroy(() => {
		closeNodeContextMenu();
		fixture.handleIntent(null);
	});
</script>

<section aria-label="Node menu fixture">
	<button data-node-id="2">Numeric</button>
	<button data-node-id="3">Choice</button>
</section>
<NodeContextMenu />
