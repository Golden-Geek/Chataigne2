import { flushSync } from 'svelte';
import { IdentifiedNodeMap } from '../../../../store/graph-node-index';
import { VersionedNodeMap } from '../../../../store/graph-index';
import { fixtureNode } from '../../../../tests/browser/fixture/session';
import {
	createCurveInspectorSelection,
	type CurveSelectionHost
} from '../nodes/curve-selection.svelte';
import type { CurveSelection } from '../nodes/curve-selection';
import type { NodeId } from '../../../../types';

export function curveBridgeFixture() {
	const owner = fixtureNode(1, 'Curve', [2, 3], 'animation_curve');
	const graph = {
		rootId: 0,
		nodesById: new IdentifiedNodeMap([
			[0, fixtureNode(0, 'Root')],
			[1, owner],
			[2, fixtureNode(2, 'A', [], 'animation_curve_key')],
			[3, fixtureNode(3, 'B', [], 'animation_curve_key')],
			[20, fixtureNode(20, 'Other')]
		]),
		parentById: new VersionedNodeMap([
			[1, 0],
			[2, 1],
			[3, 1],
			[20, 0]
		])
	};
	let local = $state<CurveSelection>({ key: null, keys: [], segments: [] });
	let ids = $state.raw<NodeId[]>([1]);
	const writes: NodeId[][] = [];
	const host = {
		status: 'connected' as CurveSelectionHost['status'],
		loading: { visible: false },
		projectGeneration: 1 as number | null,
		snapshotGeneration: 1,
		graph: { state: graph },
		get selectedNodesIds() {
			return ids;
		},
		selectNodes(next: NodeId[]) {
			writes.push([...next]);
			ids = next;
		}
	};
	let bridge!: ReturnType<typeof createCurveInspectorSelection>;
	const dispose = $effect.root(() => {
		bridge = createCurveInspectorSelection({
			get session() {
				return host;
			},
			owner,
			read: () => local,
			write: (next) => {
				local = next;
			}
		});
	});
	flushSync();
	return {
		host,
		writes,
		dispose,
		get local() {
			return local;
		},
		get ids() {
			return ids;
		},
		arm() {
			bridge.arm(new Event('pointerdown'));
		},
		select(next: NodeId[]) {
			ids = next;
			flushSync();
		},
		change(next: CurveSelection) {
			local = next;
			flushSync();
		}
	};
}

/** Effects require the browser runtime; run this audit from an owned fixture event, not SSR. */
export function auditCurveBridge() {
	const check = (condition: boolean, label: string) => {
		if (!condition) throw new Error(label);
	};
	const f = curveBridgeFixture();
	try {
		f.select([2]);
		check(
			f.local.keys.join() === '2' && f.writes.length === 0,
			'external selection projects without publishing'
		);
		f.arm();
		f.change({ key: 3, keys: [3], segments: [] });
		check(f.ids.join() === '3' && f.writes.length === 1, 'owned input publishes once');
		f.select([20]);
		f.change({ key: null, keys: [], segments: [] });
		check(
			f.ids.join() === '20' && f.writes.length === 1,
			'model cleanup cannot steal external focus'
		);
	} finally {
		f.dispose();
	}
	for (const boundary of ['project', 'snapshot', 'disconnect', 'loading'] as const) {
		const f = curveBridgeFixture();
		try {
			f.arm();
			if (boundary === 'project') f.host.projectGeneration = 2;
			if (boundary === 'snapshot') f.host.snapshotGeneration++;
			if (boundary === 'disconnect') f.host.status = 'disconnected';
			if (boundary === 'loading') f.host.loading.visible = true;
			f.change({ key: 2, keys: [2], segments: [] });
			check(f.writes.length === 0, boundary);
		} finally {
			f.dispose();
		}
	}
	return 'pass';
}
